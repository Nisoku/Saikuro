// Canonicalization utilities for parity tests.

export const canonType = (t: any): any => {
  if (!t || typeof t !== "object") return t;
  switch (t.kind) {
    case "primitive":
      return ["p", t.type];
    case "list":
      return ["l", canonType(t.item)];
    case "map":
      return ["m", canonType(t.key), canonType(t.value)];
    case "optional":
      return ["o", canonType(t.inner)];
    case "named":
      if (
        !t.name ||
        ["Object", "object", "any", "unknown", "{}"].includes(t.name)
      ) {
        return ["p", "any"];
      }
      return ["n", t.name];
    case "stream":
      return ["s", canonType(t.item)];
    case "channel":
      return ["c", canonType(t.send), canonType(t.recv)];
    default:
      return ["x", JSON.stringify(t)];
  }
};

export const canonFn = (fn: any) => {
  return {
    args: (fn.args || []).map((a: any) => {
      if (a && a.type && a.type.kind === "optional") {
        return [a.name, canonType(a.type.inner), true];
      }
      return [a.name, canonType(a.type), !!a.optional];
    }),
    returns: canonType(fn.returns),
    capabilities: (fn.capabilities || []).slice().sort(),
    visibility: fn.visibility,
    doc: "",
  };
};

export const normalizeReturns = (c: any): any => {
  if (!c) return c;
  if (Array.isArray(c)) {
    if (c[0] === "o") return normalizeReturns(c[1]);
    if (
      c[0] === "s" &&
      Array.isArray(c[1]) &&
      c[1][0] === "p" &&
      c[1][1] === "any"
    ) {
      return ["p", "any"];
    }
    return c.map(normalizeReturns);
  }
  return c;
};

export const tolerantNormalize = (a: any, b: any, c: any) => {
  const isAny = (r: any) => Array.isArray(r) && r[0] === "p" && r[1] === "any";
  const isStream = (r: any) => Array.isArray(r) && r[0] === "s";
  const isList = (r: any) => Array.isArray(r) && r[0] === "l";
  const isMap = (r: any) => Array.isArray(r) && r[0] === "m";

  const normalizeAnyComplex = (x: any, y: any) => {
    if (isAny(x) && (isStream(y) || isList(y) || isMap(y))) return ["p", "any"];
    return null;
  };

  const norm = normalizeAnyComplex(a.returns, b.returns);
  if (norm) b.returns = norm;
  const norm2 = normalizeAnyComplex(a.returns, c.returns);
  if (norm2) c.returns = norm2;
  const norm3 = normalizeAnyComplex(b.returns, a.returns);
  if (norm3) a.returns = norm3;
  const norm4 = normalizeAnyComplex(b.returns, c.returns);
  if (norm4) c.returns = norm4;
  const norm5 = normalizeAnyComplex(c.returns, a.returns);
  if (norm5) a.returns = norm5;
  const norm6 = normalizeAnyComplex(c.returns, b.returns);
  if (norm6) b.returns = norm6;

  const argCount = Math.max(a.args.length, b.args.length, c.args.length);
  for (let i = 0; i < argCount; i++) {
    const ta = a.args[i] ? a.args[i][1] : null;
    const tb = b.args[i] ? b.args[i][1] : null;
    const tc = c.args[i] ? c.args[i][1] : null;
    const anySide = isAny(ta) || isAny(tb) || isAny(tc);
    const complexSide =
      isList(ta) ||
      isMap(ta) ||
      isStream(ta) ||
      isList(tb) ||
      isMap(tb) ||
      isStream(tb) ||
      isList(tc) ||
      isMap(tc) ||
      isStream(tc);
    if (anySide && complexSide) {
      if (a.args[i]) a.args[i][1] = ["p", "any"];
      if (b.args[i]) b.args[i][1] = ["p", "any"];
      if (c.args[i]) c.args[i][1] = ["p", "any"];
    }
  }
};
