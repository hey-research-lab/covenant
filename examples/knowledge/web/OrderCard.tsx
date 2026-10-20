import { calculate_total, Order } from "./orders";

export function OrderCard({ order }: { order: Order }) {
  return <article>Total: {calculate_total(order)}</article>;
}
