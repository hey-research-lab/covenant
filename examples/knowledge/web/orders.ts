import { unitPrice } from "./pricing";

export interface Order {
  units: number;
}

export function calculate_total(order: Order): number {
  return order.units * unitPrice();
}
