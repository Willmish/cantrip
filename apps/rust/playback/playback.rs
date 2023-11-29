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
use log::info;
use log::{set_max_level, LevelFilter};
use sdk_interface::*;

const RECORD_FREQ_HZ: usize = 1_000_000; // 1MHz
const PLAY_FREQ_HZ: usize = 500_000; // .5MHz
const BUFFER_SIZE: usize = 2024; // NB: 1/2 4K page - postcard overhead

#[no_mangle]
pub fn main() {
    static mut HEAP: [u8; 4096] = [0; 4096];
    sdk_init(unsafe { &mut HEAP });
    set_max_level(LevelFilter::Info);

    info!("Audio playback demo.");

    sdk_audio_reset(
        /*rxrst=*/ true, /*txrst=*/ false, /*rxilvl=*/ 1, /*txilvl=*/ 16,
    )
    .expect("sdk_audio_reset");

    const SAMPLE_COUNT: usize = 5 * 16000;
    static mut SAMPLES: [u32; SAMPLE_COUNT] = [0u32; SAMPLE_COUNT];

    loop {
        info!("Start recording...");
        let samples_captured = record(unsafe { &mut SAMPLES });
        info!("Done recording, collected {} samples.", samples_captured);
        if samples_captured != SAMPLE_COUNT && samples_captured != 11580 {
            info!("MISSING {} samples", 11580 - (samples_captured as isize));
        }

        correct_dc_offsets(unsafe { &mut SAMPLES[..samples_captured] });
        if scale_waveform(unsafe { &mut SAMPLES[..samples_captured] }) {
            info!("Start playing {samples_captured} samples..");
            play(unsafe { &SAMPLES[..samples_captured] });
            info!("Done playing.");
        }
    }
}

fn record(samples: &mut [u32]) -> usize {
    fn first_zero(samples: &[u32]) -> Option<usize> {
        for i in 0..samples.len() {
            if samples[i] == 0 {
                return Some(i);
            }
        }
        None
    }

    sdk_audio_record_start(
        /*rate=*/ RECORD_FREQ_HZ,
        /*buffer_size=*/ BUFFER_SIZE,
        /*stop_on_full=*/ true,
    )
    .expect("sdk_audio_record_start");

    const MAX_SAMPLES_PER_READ: usize = BUFFER_SIZE / size_of::<u32>();
    let mut samples_captured: usize = 0;
    while samples_captured < samples.len() {
        let count = core::cmp::min(samples.len() - samples_captured, MAX_SAMPLES_PER_READ);
        let data_count =
            sdk_audio_record_collect(&mut samples[samples_captured..samples_captured + count])
                .expect("sdk_audio_record_collect");
        // XXX need a proper check for "no signal" and over more data
        if let Some(index) = first_zero(&samples[samples_captured..samples_captured + data_count]) {
            samples_captured += index;
            break;
        }
        samples_captured += data_count;
    }

    sdk_audio_record_stop().expect("sdk_audio_record_stop");

    samples_captured
}

fn play(samples: &[u32]) {
    sdk_audio_play_start(/*rate=*/ PLAY_FREQ_HZ, /*buffer_size=*/ BUFFER_SIZE)
        .expect("sdk_audio_play_start");

    const MAX_SAMPLES_PER_WRITE: usize = BUFFER_SIZE / size_of::<u32>();
    let mut samples_remaining = samples.len();
    let mut next_sample = 0;
    while samples_remaining > 0 {
        let count = core::cmp::min(samples_remaining, MAX_SAMPLES_PER_WRITE);
        sdk_audio_play_write(&samples[next_sample..next_sample + count])
            .expect("sdk_audio_play_write");
        next_sample += count;
        samples_remaining -= count;
    }

    sdk_audio_play_stop().expect("sdk_audio_play_stop");
}

fn correct_dc_offsets(samples: &mut [u32]) {
    let mut samples_left: [i16; 256] = [0i16; 256];
    let mut samples_right: [i16; 256] = [0i16; 256];
    let mut index_left: usize = 0;
    let mut index_right: usize = 0;
    let mut total_left: i32 = 0;
    let mut total_right: i32 = 0;

    for ix in 0..samples.len() {
        let raw_sample = samples[ix];
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

        samples[ix] = (((left - mean_left) as u32) << 16) | ((right - mean_right) as u32);
    }
}

fn scale_waveform(samples: &mut [u32]) -> bool {
    // Calculate min/max after correcting DC offsets.
    let mut max: i32 = i16::MIN as i32;
    let mut min: i32 = i16::MAX as i32;
    for i in 0..samples.len() {
        let s = (samples[i] & 0xffff) as i32;
        if s < min {
            min = s;
        }
        if s > max {
            max = s;
        }
    }
    if min == 0 && max == 0 {
        info!("Looks like silence, not playing samples...");
        return false;
    }

    // Calculate a scaling factor and apply this to scale the waveform
    // to a peak of 75% amplitude.
    let scale_max: i32 = (max * 100) / (i16::MAX as i32);
    let scale_min: i32 = (min * 100).abs() / (i16::MIN as i32);
    let scale: i32 = core::cmp::max(core::cmp::max(scale_max, scale_min), 1);
    for i in 0..samples.len() {
        let s = (samples[i] & 0xffff) as i16;
        let mut scaled_sample: i16 = ((100 * (s as i32)) / scale) as i16;
        scaled_sample = (((scaled_sample as i32) * 75) / 100) as i16;
        // Write scaled sample to both left+right channels.
        samples[i] = ((scaled_sample as u32) << 16) | (scaled_sample as u32);
    }
    true
}
