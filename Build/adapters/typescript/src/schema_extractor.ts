/**
 * TypeScript schema extraction using the TypeScript compiler API.
 *
 * Finds exported functions in source files, extracts parameter types, return
 * types, and JSDoc, and builds a schema announcement for the Saikuro runtime.
 *
 * Usage:
 *   import { SchemaExtractor } from "./schema_extractor";
 *
 *   const extractor = new SchemaExtractor();
 *   extractor.addSourceFiles(["./services/math.ts"]);
 *   const schema = extractor.buildSchema("my-namespace");
 */

/* eslint-disable @typescript-eslint/no-explicit-any */
import * as ts from "typescript";
import { readFileSync } from "fs";
import { resolve, dirname } from "path";

export interface ExtractedArg {
  name: string;
  type: TypeDescriptor;
  optional: boolean;
  defaultValue?: string | undefined;
  doc?: string | undefined;
}

export interface ExtractedFunction {
  name: string;
  args: ExtractedArg[];
  returns: TypeDescriptor;
  capabilities: string[];
  visibility: "public" | "internal" | "private";
  doc?: string | undefined;
  isAsync: boolean;
  isGenerator: boolean;
}
export type TypeDescriptor =
  | {
      kind: "primitive";
      type:
        | "bool"
        | "i32"
        | "i64"
        | "f32"
        | "f64"
        | "string"
        | "bytes"
        | "any"
        | "unit";
    }
  | { kind: "list"; item: TypeDescriptor }
  | { kind: "map"; key: TypeDescriptor; value: TypeDescriptor }
  | { kind: "optional"; inner: TypeDescriptor }
  | { kind: "named"; name: string }
  | { kind: "stream"; item: TypeDescriptor }
  | { kind: "channel"; send: TypeDescriptor; recv: TypeDescriptor };

interface JSDocInfo {
  doc?: string;
  params?: Map<string, string>;
  returns?: string;
  capability?: string;
  visibility?: "public" | "internal" | "private";
}

export class SchemaExtractor {
  private sourceFiles: Map<string, ts.SourceFile> = new Map();
  private program: ts.Program | null = null;
  private typeChecker: ts.TypeChecker | null = null;

  /**
   * Add a TypeScript source file to be analyzed.
   */
  addSourceFile(filePath: string): void {
    const resolvedPath = resolve(filePath);
    const content = readFileSync(resolvedPath, "utf-8");
    const sourceFile = ts.createSourceFile(
      resolvedPath,
      content,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TS
    );
    this.sourceFiles.set(resolvedPath, sourceFile);
  }

  /**
   * Add multiple source files.
   */
  addSourceFiles(filePaths: string[]): void {
    for (const path of filePaths) {
      this.addSourceFile(path);
    }
  }

  /**
   * Initialize the TypeScript program and type checker.
   * Must be called after adding source files and before extraction.
   */
  initialize(): void {
    if (this.sourceFiles.size === 0) {
      throw new Error("No source files added. Call addSourceFile() first.");
    }

    const rootNames = Array.from(this.sourceFiles.keys());
    const options: ts.CompilerOptions = {
      target: ts.ScriptTarget.Latest,
      module: ts.ModuleKind.ESNext,
      strict: true,
      esModuleInterop: true,
      skipLibCheck: true,
      noEmit: true,
    };

    const findSourceKey = (fileName: string): string | undefined => {
      for (const path of this.sourceFiles.keys()) {
        if (
          path === fileName ||
          path.endsWith(`/${fileName}`) ||
          path.endsWith(`\\${fileName}`) ||
          fileName.endsWith(path)
        ) {
          return path;
        }
      }
      return undefined;
    };

    this.program = ts.createProgram(rootNames, options, {
      getSourceFile: (fileName) => {
        const key = findSourceKey(fileName);
        if (key) return this.sourceFiles.get(key);
        return undefined;
      },
      writeFile: () => {},
      readFile: (fileName) => {
        // Prefer in-memory source files (exact or basename matches).
        const key = findSourceKey(fileName);
        if (key) {
          const sf = this.sourceFiles.get(key);
          if (sf) return sf.getFullText();
        }

        try {
          return ts.sys.readFile(fileName);
        } catch {
          return undefined;
        }
      },
      fileExists: (fileName) => {
        return (
          findSourceKey(fileName) !== undefined || ts.sys.fileExists(fileName)
        );
      },
      directoryExists: (dirName) => {
        try {
          return ts.sys.directoryExists(dirName);
        } catch {
          return false;
        }
      },
      getDirectories: (dirName) => ts.sys.getDirectories(dirName),
      getDefaultLibFileName: (opts) => ts.getDefaultLibFilePath(opts),
      getDefaultLibLocation: () => dirname(ts.getDefaultLibFilePath(options)),
      getCurrentDirectory: () => process.cwd(),
      getCanonicalFileName: (fileName) => fileName.toLowerCase(),
      useCaseSensitiveFileNames: () => false,
      getNewLine: () => ts.sys.newLine,
      // resolveModuleNames is the correct hook on CompilerHost; delegate to ts.resolveModuleName
      resolveModuleNames: (moduleNames: string[], containingFile: string) => {
        return moduleNames.map((moduleName) => {
          const res = ts.resolveModuleName(
            moduleName,
            containingFile,
            options,
            ts.sys as any
          );
          return res.resolvedModule || undefined;
        });
      },
    });

    this.typeChecker = this.program.getTypeChecker();
  }

  /**
   * Extract JSDoc from a node's leading trivia.
   */
  private extractJSDoc(node: ts.Node): JSDocInfo {
    const result: JSDocInfo = { params: new Map() };

    // Use the TypeScript JSDoc helpers when available to robustly parse tags.
    try {
      const tags = (ts as any).getJSDocTags
        ? ((ts as any).getJSDocTags(node as any) as ts.JSDocTag[])
        : [];

      for (const tag of tags) {
        const tagName =
          (tag.tagName && tag.tagName.getText && tag.tagName.getText()) || "";
        const comment = (tag.comment && String(tag.comment)) || "";

        if (tagName === "param") {
          // Parameter tag may have a name property
          const pName = (tag as any).name
            ? (tag as any).name.getText
              ? (tag as any).name.getText()
              : String((tag as any).name)
            : "";
          if (pName) result.params!.set(pName, comment.trim());
        } else if (tagName === "returns" || tagName === "return") {
          result.returns = comment.trim();
        } else if (tagName === "capability") {
          result.capability = comment.split(/\s+/)[0] || comment.trim();
        } else if (tagName === "visibility") {
          const v = comment.trim();
          if (v === "public" || v === "internal" || v === "private")
            result.visibility = v as any;
        }
      }
    } catch {
      // Fallthrough to text-based parsing below if helper fails
    }

    // Fallback: parse leading comment block text (conservative regex)
    const start = node.getFullStart();
    const leadingTrivia = node.getFullText().slice(0, start - node.getStart());
    const jsdocRegex = /\/\*\*([\s\S]*?)\*\//g;
    let match;
    while ((match = jsdocRegex.exec(leadingTrivia)) !== null) {
      const docText = match[1];

      // Extract @param tags
      const paramRegex = /@param\s+(?:\{([^}]+)\})?\s+(\w+)\s+-?\s*(.*)/g;
      let paramMatch;
      while ((paramMatch = paramRegex.exec(docText)) !== null) {
        result.params!.set(paramMatch[2], paramMatch[3].trim());
      }

      // Extract @returns
      const returnsMatch = docText.match(/@returns?\s*-?\s*(.*)/);
      if (returnsMatch && !result.returns) {
        result.returns = returnsMatch[1].trim();
      }

      // Extract @capability
      const capMatch = docText.match(/@capability\s+(\S+)/);
      if (capMatch && !result.capability) {
        result.capability = capMatch[1];
      }

      // Extract @visibility
      const visMatch = docText.match(/@visibility\s+(public|internal|private)/);
      if (visMatch && !result.visibility) {
        result.visibility = visMatch[1] as "public" | "internal" | "private";
      }

      // Get main description (everything except tags)
      if (!result.doc) {
        const description = docText
          .replace(/@\w+[\s\S]*?$/gm, "")
          .replace(/^\s*\*\s?/gm, "")
          .trim();
        if (description) result.doc = description;
      }
    }

    return result;
  }

  /**
   * Convert a TypeScript type to a Saikuro TypeDescriptor.
   */
  private typeToDescriptor(type: ts.Type): TypeDescriptor {
    // Handle primitive types
    const primitiveType = this.typeChecker!.typeToString(type);
    const primitiveMap: Record<
      string,
      Extract<TypeDescriptor, { kind: "primitive" }>["type"]
    > = {
      boolean: "bool",
      number: "i64", // Could be f32/f64 with more analysis
      string: "string",
      Uint8Array: "bytes",
      ArrayBuffer: "bytes",
      null: "unit",
      undefined: "unit",
      void: "unit",
      never: "unit",
    };

    if (primitiveType in primitiveMap) {
      return { kind: "primitive", type: primitiveMap[primitiveType] };
    }

    // Fallback: handle common array prints like `T[]` or `Array<T>` when
    // `isArrayType` helper wasn't available or didn't match.
    const asAny = type as any;
    if (asAny.elementType) {
      return {
        kind: "list",
        item: this.typeToDescriptor(asAny.elementType as ts.Type),
      };
    }

    if (primitiveType.endsWith("[]")) {
      const base = primitiveType.slice(0, -2).trim();
      // map base to primitive if possible
      if (base in primitiveMap) {
        return {
          kind: "list",
          item: {
            kind: "primitive",
            type: primitiveMap[base],
          } as TypeDescriptor,
        } as TypeDescriptor;
      }
      return {
        kind: "list",
        item: { kind: "named", name: base } as TypeDescriptor,
      } as TypeDescriptor;
    }

    if (primitiveType.startsWith("Array<") && primitiveType.endsWith(">")) {
      const inner = primitiveType.slice(6, -1).trim();
      if (inner in primitiveMap)
        return {
          kind: "list",
          item: {
            kind: "primitive",
            type: primitiveMap[inner],
          } as TypeDescriptor,
        } as TypeDescriptor;
      return {
        kind: "list",
        item: { kind: "named", name: inner } as TypeDescriptor,
      } as TypeDescriptor;
    }

    // Handle TypeScript Record<K,V> printed names like `Record<string, number>`
    if (primitiveType.startsWith("Record<")) {
      try {
        const ref = type as unknown as ts.TypeReference;
        const typeArgs =
          (ref as any).typeArguments || (ref as any).aliasTypeArguments;
        if (typeArgs && typeArgs.length >= 2) {
          return {
            kind: "map",
            key: this.typeToDescriptor(typeArgs[0]),
            value: this.typeToDescriptor(typeArgs[1]),
          };
        }
      } catch {
        // fallthrough to string-parse
      }

      const m = primitiveType.match(/^Record<\s*([^,>]+)\s*,\s*(.+)>$/);
      if (m) {
        const keyStr = m[1].trim();
        const valStr = m[2].trim();
        const keyDesc: TypeDescriptor =
          keyStr === "string"
            ? ({ kind: "primitive", type: "string" } as TypeDescriptor)
            : ({ kind: "named", name: keyStr } as TypeDescriptor);
        const valDesc: TypeDescriptor = {
          kind: "named",
          name: valStr,
        } as TypeDescriptor;
        return { kind: "map", key: keyDesc, value: valDesc } as TypeDescriptor;
      }
    }

    // Handle any / anonymous object types ("{}") that TS prints for empty
    // object literal types - map them to any for parity with Python.
    if (
      primitiveType === "any" ||
      primitiveType === "{}" ||
      primitiveType === "object"
    ) {
      return { kind: "primitive", type: "any" };
    }

    // Handle arrays (use isArrayType when available)
    if (this.typeChecker!.isArrayType && this.typeChecker!.isArrayType(type)) {
      const elementType = (type as any).elementType ||
        (type as any).typeArguments?.[0] || { flags: ts.TypeFlags.Any };
      return { kind: "list", item: this.typeToDescriptor(elementType) };
    }

    // Handle tuple
    if (
      (type as any).target &&
      (type as any).target.objectFlags & ts.ObjectFlags.Tuple
    ) {
      const elements =
        (type as any).resolvedTypeArguments ||
        (type as any).typeArguments ||
        [];
      const items = elements.map((e: any) =>
        this.typeToDescriptor(e as ts.Type)
      );
      return {
        kind: "list",
        item: items[0] || { kind: "primitive", type: "any" },
      };
    }

    // Handle union
    if (type.flags & ts.TypeFlags.Union) {
      const unionType = type as ts.UnionType;
      const types = unionType.types;

      // Check if it's a nullable type (T | null | undefined)
      const hasNull = types.some((t) => t.flags & ts.TypeFlags.Null);
      const hasUndefined = types.some((t) => t.flags & ts.TypeFlags.Undefined);
      const nonNullTypes = types.filter(
        (t) => !(t.flags & (ts.TypeFlags.Null | ts.TypeFlags.Undefined))
      );

      if ((hasNull || hasUndefined) && nonNullTypes.length === 1) {
        return {
          kind: "optional",
          inner: this.typeToDescriptor(nonNullTypes[0]),
        };
      }

      // For other unions, return any (complex)
      return { kind: "primitive", type: "any" };
    }

    // Handle intersection
    if (type.flags & ts.TypeFlags.Intersection) {
      // Complex - return any for now
      return { kind: "primitive", type: "any" };
    }

    // Handle object/interface
    if (type.flags & ts.TypeFlags.Object) {
      // Check if it's a Map or Record
      const symbol = type.getSymbol();
      if (symbol) {
        const name = symbol.getName();
        if (name === "Map") {
          const typeArgs = (type as ts.TypeReference).typeArguments;
          if (typeArgs && typeArgs.length >= 2) {
            return {
              kind: "map",
              key: this.typeToDescriptor(typeArgs[0]),
              value: this.typeToDescriptor(typeArgs[1]),
            };
          }
        }
        if (name === "Record") {
          const typeArgs = (type as ts.TypeReference).typeArguments;
          // Record<string, T> -> map with string keys
          if (typeArgs && typeArgs.length >= 2) {
            return {
              kind: "map",
              key: { kind: "primitive", type: "string" },
              value: this.typeToDescriptor(typeArgs[1]),
            };
          }
        }
      }

      // Check for index signature to determine if it's a map-like type
      const props = type.getProperties();
      if (props.length === 0) {
        // Empty object type -> treat as any for parity with Python
        return { kind: "primitive", type: "any" };
      }

      // For objects with properties, emit a named type using the printed
      // type name. Downstream parity tests will canonicalize shape if needed.
      return { kind: "named", name: primitiveType };
    }

    // Handle type references (generics, interfaces, types)
    if (
      (type as any).symbol &&
      (type as any).symbol.flags & ts.SymbolFlags.TypeAlias
    ) {
      // fallthrough to named
    }

    if ((type as any).typeArguments || (type as any).aliasTypeArguments) {
      const refType = type as ts.TypeReference;
      const typeArgs =
        (refType as any).typeArguments || (refType as any).aliasTypeArguments;
      const typeName = this.typeChecker!.typeToString(refType);

      if (typeName.startsWith("Promise")) {
        // Async return - look at inner type
        if (typeArgs && typeArgs.length > 0) {
          return this.typeToDescriptor(typeArgs[0]);
        }
      }

      if (
        typeName.startsWith("AsyncGenerator") ||
        typeName.startsWith("Generator")
      ) {
        // Stream return
        if (typeArgs && typeArgs.length > 0) {
          return { kind: "stream", item: this.typeToDescriptor(typeArgs[0]) };
        }
        return { kind: "stream", item: { kind: "primitive", type: "any" } };
      }

      if (typeName.startsWith("Channel")) {
        // Channel type
        if (typeArgs && typeArgs.length >= 2) {
          return {
            kind: "channel",
            send: this.typeToDescriptor(typeArgs[0]),
            recv: this.typeToDescriptor(typeArgs[1]),
          };
        }
      }
    }

    return { kind: "named", name: primitiveType };
  }

  /**
   * Extract the function signature from a function declaration.
   */
  private extractFunction(
    node: ts.FunctionDeclaration
  ): ExtractedFunction | null {
    // Skip if not exported
    const modifiers = node.modifiers;
    if (
      !modifiers ||
      !modifiers.some((m) => m.kind === ts.SyntaxKind.ExportKeyword)
    ) {
      return null;
    }

    const name = node.name?.getText();
    if (!name) return null;

    const jsdoc = this.extractJSDoc(node);

    // Extract parameters
    const args: ExtractedArg[] = [];
    const signature = this.typeChecker!.getSignatureFromDeclaration(node);
    if (signature) {
      const params = signature.parameters;
      for (let i = 0; i < params.length; i++) {
        const param = params[i];
        const decls = param.getDeclarations && param.getDeclarations();
        const paramDecl =
          Array.isArray(decls) && decls.length > 0
            ? decls[0]
            : param.valueDeclaration;
        const paramNode = paramDecl as ts.ParameterDeclaration | undefined;

        const paramName = param.getName();
        const paramType = this.typeChecker!.getTypeOfSymbolAtLocation(
          param,
          node
        );
        const isOptional = !!(
          paramNode &&
          (paramNode.questionToken !== undefined ||
            paramNode.initializer !== undefined)
        );

        args.push({
          name: paramName,
          type: this.typeToDescriptor(paramType),
          optional: isOptional,
          defaultValue:
            paramNode && paramNode.initializer
              ? paramNode.initializer.getText()
              : undefined,
          doc: jsdoc.params?.get(paramName) as string | undefined,
        });
      }

      // Extract return type
      const returnType = signature.getReturnType();
      const isGenerator =
        name.startsWith("gen_") ||
        !!node.asteriskToken ||
        ((returnType as any).symbol &&
          (returnType as any).symbol.getName &&
          (returnType as any).symbol.getName() === "AsyncGenerator");

      return {
        name,
        args,
        returns: this.typeToDescriptor(returnType),
        capabilities: jsdoc.capability ? [jsdoc.capability] : [],
        visibility: jsdoc.visibility || "public",
        doc: jsdoc.doc as string | undefined,
        isAsync: node.modifiers.some(
          (m) => m.kind === ts.SyntaxKind.AsyncKeyword
        ),
        isGenerator,
      };
    }

    return null;
  }

  /**
   * Extract functions from all added source files.
   */
  extract(): ExtractedFunction[] {
    if (!this.program || !this.typeChecker) {
      throw new Error(
        "Schema extractor not initialized. Call initialize() first."
      );
    }

    const functions: ExtractedFunction[] = [];

    // Debug: list source files available to the extractor
    // (no-op) silently proceed; keep extractor output quiet for tests

    for (const [, sourceFile] of this.sourceFiles) {
      // visit files quietly
      const visit = (node: ts.Node) => {
        if (ts.isFunctionDeclaration(node)) {
          // found function node
          const fn = this.extractFunction(node);
          if (fn) {
            functions.push(fn);
          }
        }
        ts.forEachChild(node, visit);
      };

      visit(sourceFile);
    }

    return functions;
  }

  /**
   * Build a Saikuro schema announcement from extracted functions.
   */
  buildSchema(namespace: string): object {
    const functions = this.extract();
    // Build schema without noisy debug output

    const schemaFunctions: Record<string, object> = {};
    for (const fn of functions) {
      const argList = fn.args.map((arg) => ({
        name: arg.name,
        type: arg.type,
        optional: arg.optional,
      }));

      schemaFunctions[fn.name] = {
        args: argList,
        returns: fn.isGenerator
          ? { kind: "stream", item: fn.returns }
          : fn.returns,
        visibility: fn.visibility,
        capabilities: fn.capabilities,
        ...(fn.doc && { doc: fn.doc }),
      };
    }

    return {
      version: 1,
      namespaces: {
        [namespace]: {
          functions: schemaFunctions,
        },
      },
      types: {},
    };
  }

  /** Returns the underlying TypeScript program. */
  getProgram(): ts.Program | null {
    return this.program;
  }

  /** Returns the type checker. */
  getTypeChecker(): ts.TypeChecker | null {
    return this.typeChecker;
  }
}

/** Extract schema from source files and return it as a plain object. */
export async function extractSchema(
  sourceFiles: string[],
  namespace: string
): Promise<object> {
  const extractor = new SchemaExtractor();
  extractor.addSourceFiles(sourceFiles);
  extractor.initialize();
  return extractor.buildSchema(namespace);
}
