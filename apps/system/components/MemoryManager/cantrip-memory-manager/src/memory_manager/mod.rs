// Copyright 2020 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Cantrip OS global memory management support

extern crate alloc;
use cantrip_memory_interface::MemoryLifetime;
use cantrip_memory_interface::MemoryManagerError;
use cantrip_memory_interface::MemoryManagerInterface;
use cantrip_memory_interface::MemoryManagerStats;
use cantrip_memory_interface::ObjDesc;
use cantrip_memory_interface::ObjDescBundle;
use cantrip_os_common::camkes::{seL4_CPath, Camkes};
use cantrip_os_common::sel4_sys;
use cantrip_os_common::slot_allocator;
use core::ops::Range;
use log::{debug, error, info, trace, warn};
use smallvec::SmallVec;

use sel4_sys::seL4_CNode_Delete;
use sel4_sys::seL4_CNode_Revoke;
use sel4_sys::seL4_CPtr;
use sel4_sys::seL4_Error;
use sel4_sys::seL4_Result;
use sel4_sys::seL4_UntypedDesc;
use sel4_sys::seL4_UntypedObject;
use sel4_sys::seL4_Untyped_Describe;
use sel4_sys::seL4_Untyped_Retype;
use sel4_sys::seL4_Word;

use slot_allocator::CANTRIP_CSPACE_SLOTS;

extern "Rust" {
    static SELF_CNODE: seL4_CPtr;
}

fn delete_path(path: &seL4_CPath) -> seL4_CNode_Delete {
    unsafe { seL4_CNode_Delete(path.0, path.1, path.2 as u8) }
}
fn revoke_cap(cptr: seL4_CPtr) -> seL4_Result {
    let path = Camkes::top_level_path(cptr);
    unsafe { seL4_CNode_Revoke(path.0, path.1, path.2 as u8) }
}
fn untyped_describe(cptr: seL4_CPtr) -> seL4_Untyped_Describe {
    unsafe { seL4_Untyped_Describe(cptr) }
}

// SmallVec capacity for untyped memory slabs. There are two instances;
// one for anonymous memory and one for device-backed memory. The memory
// manager is expected to be setup as a static global so these data
// structures will land in .bss and only overflow to the heap if
// initialized with more than this count.
const UNTYPED_SLAB_CAPACITY: usize = 64; // # slabs kept inline
const STATIC_UNTYPED_SLAB_CAPACITY: usize = 4; // # slabs kept inline

// The MemoryManager supports allocating & freeing seL4 objects that are
// instantiated from UntypedMemory "slabs". Allocation causes untyped memory
// to be converted to concrete types. Freeing deletes the specified capabilities
// and updates the bookkeeping. Note that a free only releases the specified
// cap; if there are dups or derived objects the memory will not be returned
// to the untyped slab from which it was allocated and the bookkeeping done
// here will be out of sync with the kernel.
// TODO(sleffler): support device-backed memory objects
#[derive(Debug)]
struct UntypedSlab {
    pub _size_bits: usize,      // NB: only used to sort
    pub free_bytes: usize,      // Available space in slab
    pub allocated_bytes: usize,  // How many bytes of memory are currently allocated
    pub allocated_objects: usize,   // Number of objects in the slab currently
    pub _base_paddr: seL4_Word, // Physical address of slab start
    pub _last_paddr: seL4_Word, // Physical address of slab end
    pub cptr: seL4_CPtr,        // seL4 untyped object
}
impl UntypedSlab {
    fn new(ut: &seL4_UntypedDesc, free_bytes: usize, cptr: seL4_CPtr) -> Self {
        UntypedSlab {
            _size_bits: ut.size_bits(),
            free_bytes,
            allocated_bytes: 0,
            allocated_objects: 0,
            _base_paddr: ut.paddr,
            _last_paddr: ut.paddr + l2tob(ut.size_bits()),
            cptr,
        }
    }
}
pub struct MemoryManager {
    untypeds: SmallVec<[UntypedSlab; UNTYPED_SLAB_CAPACITY]>,
    static_untypeds: SmallVec<[UntypedSlab; STATIC_UNTYPED_SLAB_CAPACITY]>,
    _device_untypeds: SmallVec<[UntypedSlab; UNTYPED_SLAB_CAPACITY]>,
    cur_untyped: usize,
    cur_static_untyped: usize,
    _cur_device_untyped: usize,
    //_map_cptr_untypeds: HashMap<seL4_CPtr, usize>, // Map from CNode cptr index to untypeds SmallVec index
    //_map_cptr_untypeds: SmallVec<[usize; UNTYPED_SLAB_CAPACITY]>, // Map from CNode cptr index to untypeds SmallVec index

    total_bytes: usize,     // Total available space
    allocated_bytes: usize, // Amount of space currently allocated
    requested_bytes: usize, // Amount of space allocated over all time
    overhead_bytes: usize,

    allocated_objs: usize, // # seL4 objects currently allocated
    requested_objs: usize, // # seL4 objects allocated over all time

    // Retype requests failed due to insufficient available memory.
    untyped_slab_too_small: usize,

    // Alloc requests failed due to lack of untyped memory (NB: may be
    // due to fragmentation of untyped slabs).
    out_of_memory: usize,
}

fn _howmany(value: usize, unit: usize) -> usize { value + (unit - 1) / unit }
fn _round_up(value: usize, align: usize) -> usize { _howmany(value, align) * align }

// Log2 bits to bytes.
fn l2tob(size_bits: usize) -> usize { 1 << size_bits }

impl MemoryManager {
    // Creates a new MemoryManager instance. The allocator is seeded
    // from the untyped memory descriptors.
    pub fn new(slots: Range<seL4_CPtr>, untypeds: &[seL4_UntypedDesc]) -> Self {
        assert!(!untypeds.is_empty());
        assert_eq!(slots.end - slots.start, untypeds.len());
        let mut m = MemoryManager {
            untypeds: SmallVec::new(),
            static_untypeds: SmallVec::new(),
            _device_untypeds: SmallVec::new(),
            cur_untyped: 0,
            cur_static_untyped: 0,
            _cur_device_untyped: 0,
            //_map_cptr_untypeds: HashMap::<seL4_CPtr, usize>::new(), // TODO: @Willmish possibly initialise with capacity, min length of bootinfo untypeds
            //_map_cptr_untypeds: SmallVec::new(),

            total_bytes: 0,
            allocated_bytes: 0,
            requested_bytes: 0,
            overhead_bytes: 0,

            allocated_objs: 0,
            requested_objs: 0,

            untyped_slab_too_small: 0,
            out_of_memory: 0,
        };
        for (ut_index, ut) in untypeds.iter().enumerate() {
            let ut_cptr = slots.start + ut_index;
            #[cfg(feature = "CONFIG_NOISY_UNTYPEDS")]
            log::info!("slot {} {:?}", ut_cptr, ut);
            let slab_size = l2tob(ut.size_bits());
            if ut.is_device() {
                m._device_untypeds
                    .push(UntypedSlab::new(ut, slab_size, ut_cptr));
            } else {
                if ut.is_tainted() {
                    // Slabs marked "tainted" were used by the rootserver
                    // which has terminated. Reclaim the resources with a
                    // revoke.
                    revoke_cap(slots.start + ut_index).expect("revoke untyped");
                }
                // NB: must get the current state of the slab as the value
                //   supplied by the rootserver (in |untypeds|) will reflect
                //   resources available before the above revoke.
                let info = untyped_describe(ut_cptr);
                assert_eq!(info.sizeBits, ut.size_bits());

                // We only have the remainder available for allocations.
                // Beware that slabs with existing allocations (for the
                // services constructed by the rootserver) are not generally
                // useful because we cannot recycle memory once retype'd;
                // those we carefully split to reclaim avaiilable space.
                if info.remainingBytes > 0 {
                    if info.remainingBytes == slab_size {
                        m.untypeds
                            .push(UntypedSlab::new(ut, info.remainingBytes, ut_cptr));
                    } else {
                        // Split the unallocated space into smaller slabs that
                        // are entirely unused. This is a bit tricky as the
                        // kernel allocator does implicit alignment to the slab
                        // size. We compensate for this by logically splitting
                        // the slab in 1/2 and then searching for the best slab
                        // in the smaller region. The goal here is to reclaim
                        // as much space as possible using the minimum number
                        // of slabs (to reduce overhead searching slabs when
                        // doing allocations).
                        // TODO(sleffler): move this to the rootserver
                        let size_bits = info.sizeBits - 1; // 1/2 the slab size
                                                           // Allocate alignment slabs.
                        while let Some(align_bits) = Self::find_best_slab(ut_cptr, size_bits) {
                            match Self::new_untyped(ut_cptr, align_bits) {
                                Ok(free_untyped) => {
                                    m.untypeds.push(UntypedSlab::new(
                                        // TODO: originally was ut, @Willmish need to update to reflect the size of the new Untyped!!
                                        // See: output of mdebug
                                        ut, /*XXX*/
                                        l2tob(align_bits),
                                        free_untyped,
                                    ));
                                }
                                Err(e) => {
                                    error!("Retype align {align_bits}: {e:?}")
                                }
                            }
                        }
                        // And finally allocate the 1/2-size slab.
                        match Self::new_untyped(ut_cptr, size_bits) {
                            Ok(free_untyped) => {
                                m.untypeds.push(UntypedSlab::new(
                                    ut, /*XXX*/
                                    l2tob(size_bits),
                                    free_untyped,
                                ));
                            }
                            Err(e) => {
                                error!("Retype size {size_bits}: {e:?}")
                            }
                        }
                    }
                    // XXX assumes all space in the slab is reclaimed
                    m.total_bytes += info.remainingBytes;
                } else {
                    trace!("Discard slot {ut_cptr}, size {}, no usable space", ut.size_bits());
                }

                // Use overhead to track memory allocated out of our control.
                m.overhead_bytes += slab_size - info.remainingBytes;
            }
        }
        // Sort non-device slabs by descending amount of free space.
        m.untypeds
            .sort_unstable_by(|a, b| b.free_bytes.cmp(&a.free_bytes));
        m.static_untypeds
            .sort_unstable_by(|a, b| b.free_bytes.cmp(&a.free_bytes));
        if m.static_untypeds.is_empty() {
            // Seed the pool for static object requests with the smallest
            // "normal" slab.
            m.static_untypeds.push(m.untypeds.pop().unwrap());
        }
        // Fill in the hashmap, mapping from CNode (cptr) index to an index into m.untypeds
        //for i in 0..m.untypeds.len() {
        //    // TODO: @Willmish ensure whether it is actually safe to do this: cptr should be unique, but maybe corner case?
        //    m._map_cptr_untypeds.insert(m.untypeds[i].cptr, i);
        //}
        //info!("Map cptr untypeds: {:?}", m._map_cptr_untypeds);
        m
    }

    // Total available space.
    pub fn total_available_space(&self) -> usize { self.total_bytes }
    // Current allocated space
    pub fn allocated_space(&self) -> usize { self.allocated_bytes }
    // Current free space.
    pub fn free_space(&self) -> usize { self.total_bytes - self.allocated_bytes }
    // Total space allocated over time
    pub fn total_requested_space(&self) -> usize { self.requested_bytes }
    // Current allocated space out of our control.
    pub fn overhead_space(&self) -> usize { self.overhead_bytes }

    // Current allocated objects
    pub fn allocated_objs(&self) -> usize { self.allocated_objs }
    // Total objects allocated over time
    pub fn total_requested_objs(&self) -> usize { self.requested_objs }

    pub fn untyped_slab_too_small(&self) -> usize { self.untyped_slab_too_small }
    pub fn out_of_memory(&self) -> usize { self.out_of_memory }

    // Align |base_value| according to |alignment|. This mimics the
    // alignUp logic the kernel uses for an Untyped_Retype operation.
    fn align_up(base_value: seL4_Word, alignment: seL4_Word) -> seL4_Word {
        fn bit(x: seL4_Word) -> seL4_Word { 1 << x }
        fn mask(x: seL4_Word) -> seL4_Word { bit(x) - 1 }
        (base_value + (bit(alignment) - 1)) & !mask(alignment)
    }

    // Finds the largest slab with minimum mis-alignment (if any).
    fn find_best_slab(ut_cptr: seL4_CPtr, size_bits: usize) -> Option<usize> {
        // NB: must use the current state to track each slab split
        let info = untyped_describe(ut_cptr);
        let alignment = info.remainingBytes - l2tob(size_bits);
        let mut min_mis_alignment = alignment;
        let mut best_bits = None;
        // XXX could go down to 4 (seL4_MinUntypedBits).
        for bits in (8..size_bits).rev() {
            let slab_size = l2tob(bits);
            if slab_size <= alignment {
                let free_index = l2tob(info.sizeBits) - info.remainingBytes;
                let aligned_free_index = Self::align_up(free_index, bits);
                let mis_alignment = aligned_free_index - free_index;
                if mis_alignment == 0 {
                    return Some(bits); // optimal
                }
                if mis_alignment < min_mis_alignment {
                    min_mis_alignment = mis_alignment;
                    best_bits = Some(bits);
                }
            }
        }
        if min_mis_alignment != 0 {
            warn!("Lost {min_mis_alignment} bytes due to alignment.");
        }
        best_bits
    }

    fn retype_untyped(free_untyped: seL4_CPtr, root: seL4_CPtr, obj: &ObjDesc) -> seL4_Result {
        unsafe {
            seL4_Untyped_Retype(
                free_untyped,
                /*type=*/ obj.type_.into(),
                /*size_bits=*/ obj.retype_size_bits().unwrap(),
                /*root=*/ root,
                /*node_index=*/ 0, // Ignored 'cuz depth is zero
                /*node_depth=*/ 0, // NB: store in cnode
                /*node_offset=*/ obj.cptr,
                /*num_objects=*/ obj.retype_count(),
            )
        }
    }

    fn new_untyped(src_untyped: seL4_CPtr, size_bits: usize) -> Result<seL4_CPtr, seL4_Error> {
        unsafe {
            let free_untyped = CANTRIP_CSPACE_SLOTS
                .alloc(1)
                .ok_or(seL4_Error::seL4_NotEnoughMemory)?;
            seL4_Untyped_Retype(
                src_untyped,
                /*type=*/ seL4_UntypedObject.into(),
                /*size_bytes=*/ size_bits,
                /*root=*/ SELF_CNODE,
                /*node_index=*/ 0, // NB: ignored 'cuz depth is zero
                /*node_depth=*/ 0, // NB: store in cnode
                /*node_offset*/ free_untyped,
                /*num_objects=*/ 1,
            )
            .map(|_| free_untyped)
        }
    }

    fn delete_caps(&mut self, root: seL4_CPtr, depth: u8, od: &ObjDesc) -> seL4_Result {
        for offset in 0..od.retype_count() {
            let path = (root, od.cptr + offset, depth as usize);
            let result: seL4_CNode_Delete = delete_path(&path);
            if let Err(e) =  Into::<seL4_Result>::into(Into::<seL4_Error>::into(result.error as usize)) {
                warn!("DELETE {:?} failed: od {:?} error {:?}", &path, od, e);
            }

            // Only update bookkeeping if this was the last reference and untypedSlabIndex is valid
            if result.isLastReference != 0 && result.untypedSlabIndex != 0 {
                // TODO: @Willmish - to be replaced by a hashmap or a SmallVec with cptr -> self.untypeds mapping!
                for i in 0..self.untypeds.len() {
                    if self.untypeds[i].cptr == result.untypedSlabIndex {
                        // NOTE: decrementing by one at each CNode_Delete call, since objects could potentially span multiple CNodes.
                        // This should never overflow, if object count is tracked properly
                        self.untypeds[i].allocated_objects -= 1;
                        // check if object count dropped to 0
                        if self.untypeds[i].allocated_objects == 0 {
                            // reset the slab!
                            self.untypeds[i].allocated_bytes = 0;
                        }
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn alloc_static(&mut self, bundle: &ObjDescBundle) -> Result<(), MemoryManagerError> {
        let first_ut = self.cur_static_untyped;
        let mut ut_index = first_ut;

        for od in &bundle.objs {
            // NB: we don't check slots are available (the kernel will tell us).
            while let Err(e) =
                Self::retype_untyped(self.static_untypeds[ut_index].cptr, bundle.cnode, od)
            {
                if e != seL4_Error::seL4_NotEnoughMemory {
                    // Should not happen.
                    panic!("static allocation failed: {:?}", e);
                }
                // This untyped does not have enough available space, try
                // the next slab until we exhaust all slabs. This is the best
                // we can do without per-slab bookkeeping.
                ut_index = (ut_index + 1) % self.static_untypeds.len();
                if ut_index == first_ut {
                    // TODO(sleffler): maybe steal memory from normal pool?
                    panic!("static allocation failed: out of space");
                }
            }
        }
        self.cur_static_untyped = ut_index;

        Ok(())
    }

    fn alloc_best_fit(
        &mut self,
        bundle: &ObjDescBundle,
        lifetime: MemoryLifetime,
    ) -> Result<(), MemoryManagerError> {
        //let first_ut = self.cur_untyped;
        //let mut ut_index = first_ut;

        let mut allocated_bytes: usize = 0;
        let mut allocated_objs: usize = 0;

        for od in &bundle.objs {
            // Find slab which best fits the object - naively iterate through all
            // First slab is best slab at the start
            let mut best_slab_idx: usize = 0;
            // compute first slabs left over bytes
            let mut free_index = self.untypeds[best_slab_idx].allocated_bytes;
            let mut new_slab_size: usize = Self::align_up(free_index, od.size_bits().unwrap()) + od.size_bytes().unwrap();
            let mut slab_bytes_after_alloc: usize = self.untypeds[best_slab_idx].free_bytes - new_slab_size;
            // best_slabs left over bytes
            let mut best_slab_bytes_after_alloc: usize = slab_bytes_after_alloc;
            // Identify slab which will have the smallest number of bytes left over after alloc
            for slab_idx in 1..self.untypeds.len() {
                free_index = self.untypeds[slab_idx].allocated_bytes;
                new_slab_size = Self::align_up(free_index, od.size_bits().unwrap()) + od.size_bytes().unwrap();
                // Only consider if enough space
                if new_slab_size > self.untypeds[slab_idx].free_bytes {
                    continue;
                }
                slab_bytes_after_alloc = self.untypeds[slab_idx].free_bytes - new_slab_size; 
                if slab_bytes_after_alloc < best_slab_bytes_after_alloc {
                    best_slab_idx = slab_idx;
                    best_slab_bytes_after_alloc = slab_bytes_after_alloc;
                }
                // If perfect fit, break
                if slab_bytes_after_alloc == 0 {
                    break;
                }
            }

            if let Err(e) = Self::retype_untyped(self.untypeds[best_slab_idx].cptr, bundle.cnode, od) {
                if e != seL4_Error::seL4_NoError {
                    if e != seL4_Error::seL4_NotEnoughMemory {
                        // Should not happen.
                        // TODO(sleffler): reclaim allocations
                        error!("Allocation request failed (retype returned {:?})", e);
                        return Err(MemoryManagerError::UnknownError);
                    }
                    // TODO(sleffler): reclaim allocations
                    self.out_of_memory += 1;
                    debug!("Allocation request failed (out of space)");
                    return Err(MemoryManagerError::AllocFailed);
                }
            }
            trace!("Allocated object in best slab: {:?}, wasting {:?} bytes of memory", best_slab_idx, best_slab_bytes_after_alloc);
            allocated_objs += od.retype_count();
            allocated_bytes += od.size_bytes().unwrap();
            // Update bookkeeping info for the modified slab
            self.untypeds[best_slab_idx].allocated_objects += od.retype_count();
            self.untypeds[best_slab_idx].allocated_bytes = self.untypeds[best_slab_idx].free_bytes - best_slab_bytes_after_alloc;
        }
        //self.cur_untyped = best;

        self.allocated_bytes += allocated_bytes;
        self.allocated_objs += allocated_objs;

        // NB: does not include requests that fail
        self.requested_objs += allocated_objs;
        self.requested_bytes += allocated_bytes;


        Ok(())
    }
}

impl MemoryManagerInterface for MemoryManager {
    fn alloc(
        &mut self,
        bundle: &ObjDescBundle,
        lifetime: MemoryLifetime,
    ) -> Result<(), MemoryManagerError> {
        trace!("alloc {:?} {:?}", bundle, lifetime);

        if lifetime == MemoryLifetime::Static {
            // Static allocations are handle separately.
            return self.alloc_static(bundle);
        }
        let USE_BEST_FIT = true;
        if USE_BEST_FIT {
            return self.alloc_best_fit(bundle, lifetime)
        }
        // TODO(sleffler): split by device vs no-device (or allow mixing)
        let first_ut = self.cur_untyped;
        let mut ut_index = first_ut;

        let mut allocated_bytes: usize = 0;
        let mut allocated_objs: usize = 0;

        for od in &bundle.objs {
            // NB: we don't check slots are available (the kernel will tell us).
            // TODO(sleffler): maybe check size_bytes() against untyped slab?
            //    (we depend on the kernel for now)
            while let Err(e) =
                // NB: we don't allocate ASIDPool objects but if we did it
                //   would fail because it needs to map to an UntypedObject
                Self::retype_untyped(self.untypeds[ut_index].cptr, bundle.cnode, od)
            {
                if e != seL4_Error::seL4_NotEnoughMemory {
                    // Should not happen.
                    // TODO(sleffler): reclaim allocations
                    error!("Allocation request failed (retype returned {:?})", e);
                    return Err(MemoryManagerError::UnknownError);
                }
                // This untyped does not have enough available space, try
                // the next slab until we exhaust all slabs. This is the best
                // we can do without per-slab bookkeeping.
                self.untyped_slab_too_small += 1;
                ut_index = (ut_index + 1) % self.untypeds.len();
                trace!("Advance to untyped slab {}", ut_index);
                // XXX { self.cur_untyped = ut_index; let _ = self.debug(); }
                if ut_index == first_ut {
                    // TODO(sleffler): reclaim allocations
                    self.out_of_memory += 1;
                    debug!("Allocation request failed (out of space)");
                    return Err(MemoryManagerError::AllocFailed);
                }
            }
            allocated_objs += od.retype_count();
            allocated_bytes += od.size_bytes().unwrap();
            // Update bookkeeping info for the modified slab
            let free_index = self.untypeds[ut_index].allocated_bytes;//l2tob(info.sizeBits) - info.remainingBytes;
            // TODO: @Willmish verify bit value correct:
            let aligned_free_index = Self::align_up(free_index, od.retype_size_bits().unwrap());
            self.untypeds[ut_index].allocated_objects += od.retype_count();
            self.untypeds[ut_index].allocated_bytes = aligned_free_index + od.size_bytes().unwrap();
            // OLD method of purely accumulating allocated bytes (does not account for alignment - maybe worth tracking for fragmentation stats?)
            //self.untypeds[ut_index].allocated_bytes += od.size_bytes().unwrap();
        }
        self.cur_untyped = ut_index;

        self.allocated_bytes += allocated_bytes;
        self.allocated_objs += allocated_objs;

        // NB: does not include requests that fail
        self.requested_objs += allocated_objs;
        self.requested_bytes += allocated_bytes;


        Ok(())
    }
    fn free(&mut self, bundle: &ObjDescBundle) -> Result<(), MemoryManagerError> {
        trace!("free {:?}", bundle);

        for od in &bundle.objs {
            // TODO(sleffler): support leaving objects so client can do bulk
            //   reclaim on exit (maybe require cptr != 0)
            if self.delete_caps(bundle.cnode, bundle.depth, od).is_ok() {
                // NB: atm we do not do per-untyped bookkeeping so just track
                //   global stats.
                // TODO(sleffler): temp workaround for bad bookkeeping / client mis-handling
                let size_bytes = od.size_bytes().ok_or(MemoryManagerError::ObjTypeInvalid)?;
                if size_bytes <= self.allocated_bytes {
                    self.allocated_bytes -= size_bytes;
                    self.allocated_objs -= od.retype_count();
                } else {
                    debug!("Underflow on free of {:?}", od);
                }
            }
        }
        Ok(())
    }
    fn stats(&self) -> Result<MemoryManagerStats, MemoryManagerError> {
        Ok(MemoryManagerStats {
            allocated_bytes: self.allocated_space(),
            free_bytes: self.free_space(),
            total_requested_bytes: self.total_requested_space(),
            overhead_bytes: self.overhead_space(),

            allocated_objs: self.allocated_objs(),
            total_requested_objs: self.total_requested_objs(),

            untyped_slab_too_small: self.untyped_slab_too_small(),
            out_of_memory: self.out_of_memory(),
        })
    }
    fn debug(&self) -> Result<(), MemoryManagerError> {
        // TODO(sleffler): only shows !device slabs
        let cur_cptr = self.untypeds[self.cur_untyped].cptr;
        for ut in &self.untypeds {
            let info = untyped_describe(ut.cptr);
            let size = l2tob(info.sizeBits);
            info!(target: if ut.cptr == cur_cptr { "*" } else { " " },
                "[{:2}, bits {:2}] watermark {:8} available {}, max_size_bytes {}, allocated_bytes: {}, allocated_objects: {}",
                ut.cptr,
                info.sizeBits,
                size - info.remainingBytes,
                info.remainingBytes,
                ut.free_bytes,
                ut.allocated_bytes,
                ut.allocated_objects
            );
        }
        if !self.static_untypeds.is_empty() {
            let cur_static_cptr = self.static_untypeds[self.cur_static_untyped].cptr;
            for ut in &self.static_untypeds {
                let info = untyped_describe(ut.cptr);
                let size = l2tob(info.sizeBits);
                info!(target: if ut.cptr == cur_static_cptr { "!" } else { " " },
                    "[{:2}, bits {:2}] watermark {:8} available {}",
                    ut.cptr,
                    info.sizeBits,
                    size - info.remainingBytes,
                    info.remainingBytes,
                );
            }
        }
        info!("Allocation failed on a slab: {} times. Out of memory thrown {} times.", self.untyped_slab_too_small, self.out_of_memory);
        Ok(())
    }
}
