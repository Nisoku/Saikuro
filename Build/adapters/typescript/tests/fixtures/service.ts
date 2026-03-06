/**
 * @capability calc
 * @visibility public
 * @param {number} a - first addend
 * @param {number} b - second addend
 * @returns {Promise<number>} sum
 */
export async function add(a: number, b: number): Promise<number> {
  return a + b;
}

/**
 * @visibility public
 */
export async function* gen_numbers(count: number) {
  for (let i = 0; i < count; i++) {
    yield i;
  }
}

/**
 * Optional return example
 * @returns {string | undefined}
 */
export function maybe(msg?: string): string | undefined {
  return msg;
}

/**
 * Sum values in a dictionary
 */
export function sum_values(m: Record<string, number>): number {
  let s = 0;
  for (const k of Object.keys(m)) s += m[k];
  return s;
}

/**
 * Echo list of numbers
 */
export function wrap_items(items: number[]): number[] {
  return items;
}

export type Person = {
  name: string;
  age: number;
};

export function union_echo(val: number | string): string {
  return String(val);
}

export function greet(p: Person): string {
  return `hello ${p.name}`;
}

export function optional_arg(x?: number): number | undefined {
  return x;
}
