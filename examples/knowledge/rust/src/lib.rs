pub mod pricing;

pub struct Order {
    pub units: u32,
}

pub fn calculate_total(order: &Order) -> u32 {
    pricing::unit_price() * order.units
}
