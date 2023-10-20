/*
 * Copyright 2023, Google LLC
 *
 * SPDX-License-Identifier: Apache-2.0
 */
#![no_std]
#![no_main]

use libcantrip::sdk_init;
use log::{error, info, trace};
use log::{set_max_level, LevelFilter};
use sdk_interface::*;

// NB: must match what the model uses; no way to get this out (yet)
const ENCODER_INPUT_DATA_SIZE: usize = 640;

// Audio is recorded at 1MHz
const RECORD_FREQ_HZ: usize = 1_000_000; // 1MHz

fn sleep(period: u32) {
    let _ = match sdk_timer_oneshot(/*timer=*/ 0, period) {
        Ok(_) => match sdk_timer_wait() {
            Ok(_) => {}
            Err(e) => error!("sdk_timer_wait failed: {:?}", e),
        },
        Err(e) => error!("sdk_timer_oneshot failed: {:?}", e),
    };
}

fn sdk_audio_record(data: &mut [u8]) -> Result<usize, SDKError> {
    sdk_audio_record_start(
        /*rate=*/ RECORD_FREQ_HZ,
        /*buffer_size=*/ ENCODER_INPUT_DATA_SIZE,
        /*stop_on_full=*/ true,
    )
    .expect("sdk_audio_record_start");

    fn is_silence(data: &[u8]) -> bool {
        // All-zero's is silence...
        data.iter().all(|&x| x == 0)
    }
    loop {
        let mut total_count: usize = 0;
        while total_count < data.len() {
            let data_count = sdk_audio_record_collect(&mut data[total_count..])
                .expect("sdk_audio_record_collect");
            trace!("collected {data_count} bytes of audio data");
            total_count += data_count;
            if total_count < data.len() {
                sleep(10);
            }
        }
        if is_silence(data) {
            info!("silence")
        } else {
            break;
        }
    }

    sdk_audio_record_stop().expect("sdk_audio_record_stop");

    Ok(data.len())
}

#[no_mangle]
pub fn main() {
    static mut HEAP: [u8; 4096] = [0; 4096];
    sdk_init(unsafe { &mut HEAP });
    set_max_level(LevelFilter::Info);

    let model_name = "soundstream_encoder_non_streaming.kelvin";

    info!("Soundstream demo using {model_name}.");

    // Run the model once so it's loaded.
    sdk_model_oneshot(model_name).expect("sdk_model_oneshot");
    sdk_model_wait().expect("sdk_model_wait");
    let (model_id, model_input) = sdk_model_get_input_params(model_name).expect(model_name);
    trace!("{model_name} loaded: {:x?}", &model_input);
    // XXX verify model_input.input_ptr & model_input.input_size_bytes

    let mut model_running = false;

    loop {
        if !model_running {
            let mut audio_data: [u8; ENCODER_INPUT_DATA_SIZE] = [0u8; ENCODER_INPUT_DATA_SIZE]; // XXX MaybeUninit
            let data_count = sdk_audio_record(&mut audio_data).expect("sdk_audio_record");
            if data_count > 0 {
                // Write raw i2s data to the model's input data region.
                // TODO(sleffler): bypass app when data format is compatible w/ model input?
                sdk_model_get_input_params(model_name).expect("sdk_model_get_input_params"); // XXX force model load
                info!("input {:x?}", &audio_data[..data_count]);
                match sdk_model_set_input(
                    model_id,
                    /*input_data_offset=*/ 0,
                    &audio_data[..data_count],
                ) {
                    Ok(_) => {
                        // Start the model running, the calls to
                        // sdk_model_output (below) effectively poll for
                        // completion.
                        // (do we need to wait for a specific amount of i2s data or period of time?).
                        if let Err(e) = sdk_model_oneshot(model_name) {
                            panic!("Oneshot {model_name} failed: {:?}", e);
                        } else {
                            model_running = true;
                            trace!("model is running");
                        }
                    }
                    Err(SDKRuntimeError::SDKNoSuchModel) => sleep(1000),
                    Err(e) => panic!("sdk_model_write_input: {:?}", e),
                }
            }
        }
        if model_running {
            // Fetch output and send through uart.
            match sdk_model_output(model_id) {
                Ok(output) => {
                    if output.return_code == 0 {
                        // Send encoder output to the UART base64-encoded.
                        use base64ct::{Base64, Encoding};
                        info!("ENCODER:{}", &Base64::encode_string(&output.data));
                    } else {
                        // Model run failed, how should this be handled?
                        trace!("model returns {}", output.return_code);
                    }
                    model_running = false;
                    trace!("model is not running");
                }
                Err(SDKRuntimeError::SDKNoModelOutput) => sleep(1000),
                Err(e) => info!("no model output: {:?}", e),
            }
        }
    }
}
