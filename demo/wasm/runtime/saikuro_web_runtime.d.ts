/* tslint:disable */
/* eslint-disable */

export function start_runtime(channel: string): Promise<void>;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly start_runtime: (a: number, b: number) => any;
    readonly wasm_bindgen_2474f29060f80a46___convert__closures_____invoke___wasm_bindgen_2474f29060f80a46___JsValue__core_9b3796e30d99ddb7___result__Result_____wasm_bindgen_2474f29060f80a46___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_2474f29060f80a46___convert__closures_____invoke___js_sys_91987e62a167b1f4___Function_fn_wasm_bindgen_2474f29060f80a46___JsValue_____wasm_bindgen_2474f29060f80a46___sys__Undefined___js_sys_91987e62a167b1f4___Function_fn_wasm_bindgen_2474f29060f80a46___JsValue_____wasm_bindgen_2474f29060f80a46___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_2474f29060f80a46___convert__closures_____invoke___web_sys_47430de0600c93a4___features__gen_CloseEvent__CloseEvent______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_2474f29060f80a46___convert__closures_____invoke_______true_: (a: number, b: number) => void;
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
