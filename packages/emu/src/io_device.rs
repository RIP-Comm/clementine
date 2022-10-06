pub trait IoDevice {
    type Address;
    type Value;

    fn read_at(&self, address: Self::Address) -> Self::Value;
    fn write_at(&mut self, address: Self::Address, value: Self::Value);
}
