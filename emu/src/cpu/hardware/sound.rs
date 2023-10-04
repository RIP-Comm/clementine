use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Sound {
    pub channel1_sweep: u16,
    pub channel1_duty_length_envelope: u16,
    pub channel1_frequency_control: u16,
    pub channel2_duty_length_envelope: u16,
    pub channel2_frequency_control: u16,
    pub channel3_stop_wave_ram_select: u16,
    pub channel3_length_volume: u16,
    pub channel3_frequency_control: u16,
    pub channel4_length_envelope: u16,
    pub channel4_frequency_control: u16,
    pub control_stereo_volume_enable: u16,
    pub control_mixing_dma_control: u16,
    pub control_sound_on_off: u16,
    pub sound_pwm_control: u16,
    pub channel3_wave_pattern_ram: [u8; 16],
    pub channel_a_fifo: u32,
    pub channel_b_fifo: u32,
}
