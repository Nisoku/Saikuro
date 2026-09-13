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

/**
 * JS entry point: start the runtime, listening for adapters that
 * rendezvous on `channel`. The accept loop and peer wiring live in
 * `saikuro_runtime::wasm::start_runtime`.
 */
export function start_runtime(channel: string): Promise<void>;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly pump: () => void;
    readonly start: (a: number, b: number) => void;
    readonly start_runtime: (a: number, b: number) => any;
    readonly _critical_section_1_0_acquire: () => void;
    readonly _critical_section_1_0_release: () => void;
    readonly __embassy_time_queue_item_from_waker: (a: number) => number;
    readonly __pender: (a: number) => void;
    readonly __try_embassy_time_queue_item_from_waker: (a: number) => number;
    readonly wasm_bindgen_a4bf661739f41fea___convert__closures_____invoke___js_sys_6f6125ef71bc6070___Function_fn_wasm_bindgen_a4bf661739f41fea___JsValue_____wasm_bindgen_a4bf661739f41fea___sys__Undefined___js_sys_6f6125ef71bc6070___Function_fn_wasm_bindgen_a4bf661739f41fea___JsValue_____wasm_bindgen_a4bf661739f41fea___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_a4bf661739f41fea___convert__closures_____invoke___wasm_bindgen_a4bf661739f41fea___JsValue__core_ed718c3d60ebd546___result__Result_____wasm_bindgen_a4bf661739f41fea___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_a4bf661739f41fea___convert__closures_____invoke___web_sys_378c41a30e6ef8bb___features__gen_MessageEvent__MessageEvent______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_a4bf661739f41fea___convert__closures_____invoke_______true_: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
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
