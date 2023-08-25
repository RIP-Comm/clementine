#[derive(Default)]
pub struct Serial {
    // This is SIODATA32 when single-player mode or two different 16bits registers in multiplayer mode
    // SIOMULTI0 and SIOMULTI1
    pub sio_data_32_multi_data_0_data_1: u32,
    pub sio_multi_data_2: u16,
    pub sio_multi_data_3: u16,
    pub sio_control_register: u16,
    // This is SIOMLT_SEND and SIODATA8
    pub sio_multi_data_send_data_8: u16,
    pub sio_mode_select: u16,
    pub infrared_register: u16,
    pub sio_joy_bus_control: u16,
    pub sio_joy_bus_receive_data: u32,
    pub sio_joy_bus_transmit_data: u32,
    pub sio_joy_bus_receive_status: u16,
}
