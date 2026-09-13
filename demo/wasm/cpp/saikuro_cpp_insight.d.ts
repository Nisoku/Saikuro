/* tslint:disable */
/* eslint-disable */

/**
 * JS entry point: pump the executor once from the browser event loop.
 */
export function pump(): void;

/**
 * JS entry point: start the runtime on `channel`.
 */
export function start(channel: string): void;

export function start_cpp_provider(channel: string): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly pump: () => void;
    readonly saikuro_provider_new: (a: number) => number;
    readonly saikuro_provider_register_with_schema: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly saikuro_provider_serve_async: (a: number, b: number, c: number, d: number) => void;
    readonly saikuro_string_dup: (a: number) => number;
    readonly saikuro_last_error_message: () => number;
    readonly saikuro_provider_free: (a: number) => void;
    readonly saikuro_provider_register: (a: number, b: number, c: number, d: number) => number;
    readonly _critical_section_1_0_acquire: () => void;
    readonly _critical_section_1_0_release: () => void;
    readonly saikuro_string_free: (a: number) => void;
    readonly start: (a: number, b: number) => void;
    readonly start_cpp_provider: (a: number, b: number) => void;
    readonly __embassy_time_queue_item_from_waker: (a: number) => number;
    readonly __pender: (a: number) => void;
    readonly __try_embassy_time_queue_item_from_waker: (a: number) => number;
    readonly wasm_bindgen_2a67c6f173b08fad___convert__closures_____invoke___web_sys_c4668fa48e45aa0b___features__gen_MessageEvent__MessageEvent______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_2a67c6f173b08fad___convert__closures_____invoke_______true_: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
