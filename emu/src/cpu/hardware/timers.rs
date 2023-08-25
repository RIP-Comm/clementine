#[derive(Default)]
pub struct Timers {
    /// Timer 0 Counter/Reload
    pub tm0cnt_l: u16,
    /// Timer 0 Control
    pub tm0cnt_h: u16,
    /// Timer 1 Counter/Reload
    pub tm1cnt_l: u16,
    /// Timer 1 Control
    pub tm1cnt_h: u16,
    /// Timer 2 Counter/Reload
    pub tm2cnt_l: u16,
    /// Timer 2 Control
    pub tm2cnt_h: u16,
    /// Timer 3 Counter/Reload
    pub tm3cnt_l: u16,
    /// Timer 3 Control
    pub tm3cnt_h: u16,
}
