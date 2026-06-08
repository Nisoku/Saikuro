/* tslint:disable */
/* eslint-disable */

export function start_cpp_provider(channel: string): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly start_cpp_provider: (a: number, b: number) => void;
    readonly saikuro_provider_new: (a: number) => number;
    readonly saikuro_provider_register_with_schema: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly saikuro_provider_serve: (a: number, b: number) => number;
    readonly saikuro_string_dup: (a: number) => number;
    readonly saikuro_channel_abort: (a: number) => number;
    readonly saikuro_channel_close: (a: number) => number;
    readonly saikuro_channel_free: (a: number) => void;
    readonly saikuro_channel_next_json: (a: number, b: number, c: number) => number;
    readonly saikuro_channel_send_json: (a: number, b: number) => number;
    readonly saikuro_client_batch_json: (a: number, b: number) => number;
    readonly saikuro_client_call_json: (a: number, b: number, c: number) => number;
    readonly saikuro_client_call_json_timeout: (a: number, b: number, c: number, d: number) => number;
    readonly saikuro_client_cast_json: (a: number, b: number, c: number) => number;
    readonly saikuro_client_channel_json: (a: number, b: number, c: number) => number;
    readonly saikuro_client_close: (a: number) => number;
    readonly saikuro_client_connect: (a: number) => number;
    readonly saikuro_client_free: (a: number) => void;
    readonly saikuro_client_log: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly saikuro_client_resource_json: (a: number, b: number, c: number) => number;
    readonly saikuro_client_stream_json: (a: number, b: number, c: number) => number;
    readonly saikuro_last_error_message: () => number;
    readonly saikuro_provider_close: (a: number) => number;
    readonly saikuro_provider_free: (a: number) => void;
    readonly saikuro_provider_register: (a: number, b: number, c: number, d: number) => number;
    readonly saikuro_stream_free: (a: number) => void;
    readonly saikuro_stream_next_json: (a: number, b: number, c: number) => number;
    readonly saikuro_string_free: (a: number) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h29db406d58a36a18: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__h46d954d36a152692: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h46d954d36a152692_2: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hc65fe80513b2e744: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h46d954d36a152692_4: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hc3fc4ed0293d2be8: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
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
