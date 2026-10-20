from python.pricing import unit_price


def calculate_total(units):
    return units * unit_price()


class Order:
    def __init__(self, units):
        self.units = units

    def total(self):
        return calculate_total(self.units)
