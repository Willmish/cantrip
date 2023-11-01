/*
 * Copyright 2023, Google LLC
 *
 * SPDX-License-Identifier: Apache-2.0
 */
#![no_std]
#![no_main]

// Test playback app. Based on examples/i2s_record_playback.c.

use core::mem::size_of;
use libcantrip::sdk_init;
use log::{info, trace};
use log::{set_max_level, LevelFilter};
use sdk_interface::*;

#[no_mangle]
pub fn main() {
    static mut HEAP: [u8; 4096] = [0; 4096];
    sdk_init(unsafe { &mut HEAP });
    set_max_level(LevelFilter::Info);

    const RECORD_FREQ_HZ: usize = 1_000_000; // 1MHz
    const PLAY_FREQ_HZ: usize = 500_000; // .5MHz
                                         //    const SAMPLE_COUNT: usize = 5 * RECORD_FREQ_HZ; // 5s of data XXX not enough memory
    const SAMPLE_COUNT: usize = 16000; // XXX hack
    const BUFFER_SIZE: usize = 2048; // NB: 1/2 4K page used for RPC's
    static mut SAMPLES: [u32; SAMPLE_COUNT] = [0u32; SAMPLE_COUNT];

    let mut samples_left: [i16; 256] = [0i16; 256];
    let mut samples_right: [i16; 256] = [0i16; 256];
    let mut index_left: usize = 0;
    let mut index_right: usize = 0;
    let mut total_left: i32 = 0;
    let mut total_right: i32 = 0;

    info!("Audio playback demo.");

    sdk_audio_reset(
        /*rxrst=*/ true, /*txrst=*/ false, /*rxilvl=*/ 1, /*txilvl=*/ 16,
    )
    .expect("sdk_audio_reset");

    loop {
        unsafe {
            info!("Start recording...");

            sdk_audio_record_start(
                /*rate=*/ RECORD_FREQ_HZ,
                /*buffer_size=*/ BUFFER_SIZE,
                /*stop_on_full=*/ true,
            )
            .expect("sdk_audio_record_start");

            let mut sample: usize = 0;
            while sample < SAMPLES.len() {
                /*
                            // XXX maybe combine wait & collect
                            // Wait for record buffer to be at least 1/2 full.
                            sdk_audio_record_wait(BUFFER_SIZE / 2).expect("sdk_audio_record_wait");
                */
                let mut data: [u8; 1024] = [0u8; 1024]; // XXX
                let data_count =
                    sdk_audio_record_collect(&mut data).expect("sdk_audio_record_collect");
                trace!("collected {data_count} bytes of audio data");

                assert!((data_count % size_of::<u32>()) == 0);
                for ix in 0..(data_count / size_of::<u32>()) {
                    let raw_sample: u32 = unsafe { data.as_ptr().cast::<u32>().add(ix).read() };
                    let left = (raw_sample >> 16) as u16;
                    let right = (raw_sample & 0xffff) as u16;
                    total_left -= samples_left[index_left] as i32;
                    total_right -= samples_right[index_right] as i32;
                    total_left += left as i32;
                    samples_left[index_left] = left as i16;
                    total_right += right as i32;
                    samples_right[index_right] = right as i16;
                    index_left = (index_left + 1) % samples_left.len();
                    index_right = (index_right + 1) % samples_right.len();
                    let mean_left: u16 = (total_left as usize / samples_left.len()) as u16;
                    let mean_right: u16 = (total_right as usize / samples_right.len()) as u16;

                    SAMPLES[sample] =
                        (((left - mean_left) as u32) << 16) | ((right - mean_right) as u32);
                    sample += 1;
                    if sample == SAMPLES.len() {
                        break;
                    }
                }
            }

            // Disable recording and discard any buffered data.
            // XXX consume collected data if space available?
            sdk_audio_record_stop().expect("sdk_audio_record_stop");
            let samples_captured: usize = sample;

            info!("Done recording, collected {} samples.", samples_captured);

            // Calculate min/max after correcting DC offsets.
            let mut max: i32 = i16::MIN as i32;
            let mut min: i32 = i16::MAX as i32;
            for i in 0..samples_captured {
                let s = (SAMPLES[i] >> 16) as i32; // XXX >>16 or &0xffff?
                if s < min {
                    min = s;
                }
                if s > max {
                    max = s;
                }
            }
            if min == 0 && max == 0 {
                info!("Looks like silence, not playing samples...");
                continue;
            }

            // Calculate a scaling factor and apply this to scale the waveform
            // to a peak of 75% amplitude.
            let scale_max: i32 = (max * 100) / (i16::MAX as i32);
            let scale_min: i32 = (min * 100).abs() / (i16::MIN as i32);
            let scale: i32 = core::cmp::max(core::cmp::max(scale_max, scale_min), 1);
            for i in 0..samples_captured {
                let s = (SAMPLES[i] >> 16) as i16; // XXX >>16 or &0xffff?
                let scaled_sample: i16 = ((100 * (s as i32)) / scale) as i16;
                SAMPLES[i] = ((75 * (scaled_sample as i32)) / 100) as u32;
            }

            info!("Playing recorded samples..");

            sdk_audio_play_start(/*rate=*/ PLAY_FREQ_HZ, /*buffer_size=*/ BUFFER_SIZE)
                .expect("sdk_audio_play_start");

            const MAX_SAMPLES_PER_WRITE: usize = BUFFER_SIZE / size_of::<u32>();
            let mut samples_remaining = samples_captured;
            let mut next_sample = 0;
            while samples_remaining > 0 {
                let count = core::cmp::min(samples_remaining, MAX_SAMPLES_PER_WRITE);
                sdk_audio_play_write(core::mem::transmute(
                    &SAMPLES[next_sample..next_sample + count],
                ))
                .expect("sdk_audio_play_write");
                next_sample += count;
                samples_remaining -= count;
            }

            sdk_audio_play_stop().expect("sdk_audio_play_stop");

            info!("Done playing.");
        }
    }
}
