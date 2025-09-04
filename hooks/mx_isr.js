// @name: MX_ISR
// @min_args: 1
// @max_args: 2
// @description: Calculates ISR tax in mexico based on the monto and period
// @example: MX_ISR(21000, "semanal") returns 2817.603008

const isr = {
  "semanal": [
    { "inferior": 0.01, "superior": 171.78, "fijo": 0.00, "porcentaje": 1.92 },
    { "inferior": 171.79, "superior": 1458.03, "fijo": 3.29, "porcentaje": 6.40 },
    { "inferior": 1458.04, "superior": 2562.35, "fijo": 85.61, "porcentaje": 10.88 },
    { "inferior": 2562.36, "superior": 2978.64, "fijo": 205.80, "porcentaje": 16.00 },
    { "inferior": 2978.65, "superior": 3566.22, "fijo": 272.37, "porcentaje": 17.92 },
    { "inferior": 3566.23, "superior": 7192.64, "fijo": 377.65, "porcentaje": 21.36 },
    { "inferior": 7192.65, "superior": 11336.57, "fijo": 1152.27, "porcentaje": 23.52 },
    { "inferior": 11336.58, "superior": 21643.30, "fijo": 2126.95, "porcentaje": 30.00 },
    { "inferior": 21643.31, "superior": 28857.78, "fijo": 5218.92, "porcentaje": 32.00 },
    { "inferior": 28857.79, "superior": 86573.34, "fijo": 7527.59, "porcentaje": 34.00 },
    { "inferior": 86573.35, "superior": null, "fijo": 27150.83, "porcentaje": 35.00 }
  ],
  "quincenal": [
    { "inferior": 0.01, "superior": 368.10, "fijo": 0.00, "porcentaje": 1.92 },
    { "inferior": 368.11, "superior": 3124.35, "fijo": 7.05, "porcentaje": 6.40 },
    { "inferior": 3124.36, "superior": 5490.75, "fijo": 183.45, "porcentaje": 10.88 },
    { "inferior": 5490.76, "superior": 6382.80, "fijo": 441.00, "porcentaje": 16.00 },
    { "inferior": 6382.81, "superior": 7641.90, "fijo": 583.65, "porcentaje": 17.92 },
    { "inferior": 7641.91, "superior": 15412.80, "fijo": 809.25, "porcentaje": 21.36 },
    { "inferior": 15412.81, "superior": 24292.65, "fijo": 2469.15, "porcentaje": 23.52 },
    { "inferior": 24292.66, "superior": 46378.50, "fijo": 4557.75, "porcentaje": 30.00 },
    { "inferior": 46378.51, "superior": 61838.10, "fijo": 11183.40, "porcentaje": 32.00 },
    { "inferior": 61838.11, "superior": 185514.30, "fijo": 16130.55, "porcentaje": 34.00 },
    { "inferior": 185514.31, "superior": null, "fijo": 58180.35, "porcentaje": 35.00 }
  ],
  "mensual": [
    { "inferior": 0.01, "superior": 746.04, "fijo": 0.00, "porcentaje": 1.92 },
    { "inferior": 746.05, "superior": 6332.05, "fijo": 14.32, "porcentaje": 6.40 },
    { "inferior": 6332.06, "superior": 11128.01, "fijo": 371.83, "porcentaje": 10.88 },
    { "inferior": 11128.02, "superior": 12935.82, "fijo": 893.63, "porcentaje": 16.00 },
    { "inferior": 12935.83, "superior": 15487.71, "fijo": 1182.88, "porcentaje": 17.92 },
    { "inferior": 15487.72, "superior": 31236.49, "fijo": 1640.18, "porcentaje": 21.36 },
    { "inferior": 31236.50, "superior": 49233.00, "fijo": 5004.12, "porcentaje": 23.52 },
    { "inferior": 49233.01, "superior": 93993.90, "fijo": 9236.89, "porcentaje": 30.00 },
    { "inferior": 93993.91, "superior": 125325.20, "fijo": 22665.17, "porcentaje": 32.00 },
    { "inferior": 125325.21, "superior": 375975.61, "fijo": 32691.18, "porcentaje": 34.00 },
    { "inferior": 375975.62, "superior": null, "fijo": 117912.32, "porcentaje": 35.00 }
  ]
};

function execute(args) {
    const monto = args[0];
    const period = args[1];

    const isrTable = isr[period];

    const isrRange = isrTable.find(
      (row) =>
        row.inferior <= monto &&
        (row.superior >= monto || row.superior === null)
    );

    const exedente = monto - isrRange.inferior;
    const impuesto_isr = isrRange.fijo + (exedente * (isrRange.porcentaje / 100))
    const response = {
      rango: isrRange,
      isr: impuesto_isr,
      neto: monto - impuesto_isr
    };
    return response;
}