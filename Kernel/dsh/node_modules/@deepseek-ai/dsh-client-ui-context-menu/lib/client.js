window.__ModuleLoader__.load({
	id: "@deepseek-ai/dsh-client-ui-context-menu",
	factory: (require) => {
		var module = { exports: {} };
		var exports = module.exports;
		Object.defineProperty(exports, Symbol.toStringTag, { value: "Module" });
		let _deepseek_ai_dsh_client_store = require("@deepseek-ai/dsh-client-store");
		let _deepseek_ai_dsh_client_ui_primitives = require("@deepseek-ai/dsh-client-ui-primitives");
		let react_jsx_runtime = require("react/jsx-runtime");
		//#region ../../../vendor/cosmokit/src/misc.ts
		/** Return true when a value is `null` or `undefined`. */
		function isNullable(value) {
			return value === null || value === void 0;
		}
		/** Return true for non-array object values. */
		function isPlainObject(data) {
			return data && typeof data === "object" && !Array.isArray(data);
		}
		/** Filter object entries and return a new object. */
		function filterKeys(object, filter) {
			return Object.fromEntries(Object.entries(object).filter(([key, value]) => filter(key, value)));
		}
		/** Map object values while preserving the original key set. */
		function mapValues(object, transform) {
			return Object.fromEntries(Object.entries(object).map(([key, value]) => [key, transform(value, key)]));
		}
		/** Pick selected keys from an object, optionally including `undefined` values. */
		function pick(source, keys, forced) {
			if (!keys) return { ...source };
			const result = {};
			for (const key of keys) if (forced || source[key] !== void 0) result[key] = source[key];
			return result;
		}
		//#endregion
		//#region ../../../vendor/cosmokit/src/volatile.ts
		/** Shared config references used by schema validators and plugin runtimes. */
		const write = Symbol.for("cosmokit.volatile.write");
		function snapshot(value, ancestors = /* @__PURE__ */ new Set()) {
			if (typeof value === "function") throw new TypeError("volatile config cannot contain functions");
			if (value === null || typeof value !== "object") return value;
			if (ancestors.has(value)) throw new TypeError("volatile config cannot contain cycles");
			ancestors.add(value);
			try {
				if (Array.isArray(value)) return Object.freeze(value.map((item) => snapshot(item, ancestors)));
				if (Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null) throw new TypeError("volatile config objects must be plain objects or arrays");
				return Object.freeze(Object.fromEntries(Object.entries(value).map(([key, item]) => [key, snapshot(item, ancestors)])));
			} finally {
				ancestors.delete(value);
			}
		}
		/**
		* Create a detached reference containing an immutable copy of the supplied data.
		* @param value - validated config data; class instances and functions are unsupported.
		* @returns a reference whose value is updated only by its owning runtime.
		*/
		function createVolatile(value) {
			let current = snapshot(value);
			return Object.freeze({
				get: () => current,
				[write]: (value) => {
					current = value;
				}
			});
		}
		/**
		* Identify references across ESM/CJS copies of the shared library.
		* @param value - a parsed config value.
		* @returns whether the value implements the shared reference protocol.
		*/
		function isVolatile(value) {
			return typeof value === "object" && value !== null && write in value;
		}
		//#endregion
		//#region ../../../vendor/cosmokit/src/types.ts
		/** Test values using `instanceof` with a `toStringTag` fallback. */
		function is(type, value) {
			if (arguments.length === 1) return (value) => is(type, value);
			return type in globalThis && value instanceof globalThis[type] || Object.prototype.toString.call(value).slice(8, -1) === type;
		}
		function isArrayBufferLike(value) {
			return is("ArrayBuffer", value) || is("SharedArrayBuffer", value);
		}
		function isArrayBufferSource(value) {
			return isArrayBufferLike(value) || ArrayBuffer.isView(value);
		}
		let Binary;
		(function(_Binary) {
			_Binary.is = isArrayBufferLike;
			_Binary.isSource = isArrayBufferSource;
			function fromSource(source) {
				if (ArrayBuffer.isView(source)) return source.buffer.slice(source.byteOffset, source.byteOffset + source.byteLength);
				else return source;
			}
			_Binary.fromSource = fromSource;
			function toBase64(source) {
				source = fromSource(source);
				if (typeof Buffer !== "undefined") return Buffer.from(source).toString("base64");
				let binary = "";
				const bytes = new Uint8Array(source);
				for (let i = 0; i < bytes.byteLength; i++) binary += String.fromCharCode(bytes[i]);
				return btoa(binary);
			}
			_Binary.toBase64 = toBase64;
			function fromBase64(source) {
				if (typeof Buffer !== "undefined") return fromSource(Buffer.from(source, "base64"));
				return Uint8Array.from(atob(source), (c) => c.charCodeAt(0));
			}
			_Binary.fromBase64 = fromBase64;
			function toHex(source) {
				source = fromSource(source);
				if (typeof Buffer !== "undefined") return Buffer.from(source).toString("hex");
				return Array.from(new Uint8Array(source), (byte) => byte.toString(16).padStart(2, "0")).join("");
			}
			_Binary.toHex = toHex;
			function fromHex(source) {
				if (typeof Buffer !== "undefined") return fromSource(Buffer.from(source, "hex"));
				const hex = source.length % 2 === 0 ? source : source.slice(0, source.length - 1);
				const buffer = [];
				for (let i = 0; i < hex.length; i += 2) buffer.push(parseInt(`${hex[i]}${hex[i + 1]}`, 16));
				return Uint8Array.from(buffer).buffer;
			}
			_Binary.fromHex = fromHex;
		})(Binary || (Binary = {}));
		Binary.fromBase64;
		Binary.toBase64;
		Binary.fromHex;
		Binary.toHex;
		/** Deep-clone common JavaScript values while preserving prototypes and cycles. */
		function clone(source, refs = /* @__PURE__ */ new Map()) {
			if (!source || typeof source !== "object") return source;
			if (is("Date", source)) return new Date(source.valueOf());
			if (is("RegExp", source)) return new RegExp(source.source, source.flags);
			if (isArrayBufferLike(source)) return source.slice(0);
			if (ArrayBuffer.isView(source)) return source.buffer.slice(source.byteOffset, source.byteOffset + source.byteLength);
			const cached = refs.get(source);
			if (cached) return cached;
			if (Array.isArray(source)) {
				const result = [];
				refs.set(source, result);
				source.forEach((value, index) => {
					result[index] = Reflect.apply(clone, null, [value, refs]);
				});
				return result;
			}
			const result = Object.create(Object.getPrototypeOf(source));
			refs.set(source, result);
			for (const key of Reflect.ownKeys(source)) {
				const descriptor = { ...Reflect.getOwnPropertyDescriptor(source, key) };
				if ("value" in descriptor) descriptor.value = Reflect.apply(clone, null, [descriptor.value, refs]);
				Reflect.defineProperty(result, key, descriptor);
			}
			return result;
		}
		/**
		* Compare values recursively, treating two volatile references as equal regardless of value.
		* Strict comparison distinguishes null/undefined, treats opaque objects by identity,
		* compares URLs by normalized href, treats array holes as undefined, and considers distinct cyclic structures unequal.
		* @param a - first value.
		* @param b - second value.
		* @param strict - whether to require strict data equality outside volatile references.
		* @returns whether the values compare equal.
		*/
		function deepEqual(a, b, strict) {
			const ancestors = /* @__PURE__ */ new Set();
			function compare(a, b) {
				if (a === b) return true;
				if (isVolatile(a) || isVolatile(b)) return isVolatile(a) && isVolatile(b);
				if (!strict && isNullable(a) && isNullable(b)) return true;
				if (typeof a !== typeof b || typeof a !== "object" || !a || !b) return false;
				if (ancestors.has(a)) return false;
				function check(test, then) {
					return test(a) ? test(b) ? then(a, b) : false : test(b) ? false : void 0;
				}
				ancestors.add(a);
				try {
					return check(Array.isArray, (a, b) => {
						if (a.length !== b.length) return false;
						for (let index = 0; index < a.length; index++) if (!compare(a[index], b[index])) return false;
						return true;
					}) ?? check(is("Date"), (a, b) => a.valueOf() === b.valueOf()) ?? check(is("URL"), (a, b) => a.href === b.href) ?? check(is("RegExp"), (a, b) => a.source === b.source && a.flags === b.flags) ?? check(isArrayBufferLike, (a, b) => {
						if (a.byteLength !== b.byteLength) return false;
						const viewA = new Uint8Array(a);
						const viewB = new Uint8Array(b);
						for (let i = 0; i < viewA.length; i++) if (viewA[i] !== viewB[i]) return false;
						return true;
					}) ?? ((!strict || [a, b].every((value) => Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null)) && Object.keys({
						...a,
						...b
					}).every((key) => compare(a[key], b[key])));
				} finally {
					ancestors.delete(a);
				}
			}
			return compare(a, b);
		}
		//#endregion
		//#region ../../../vendor/cosmokit/src/time.ts
		let Time;
		(function(_Time) {
			_Time.millisecond = 1;
			const second = _Time.second = 1e3;
			const minute = _Time.minute = second * 60;
			const hour = _Time.hour = minute * 60;
			const day = _Time.day = hour * 24;
			const week = _Time.week = day * 7;
			let timezoneOffset = (/* @__PURE__ */ new Date()).getTimezoneOffset();
			function setTimezoneOffset(offset) {
				timezoneOffset = offset;
			}
			_Time.setTimezoneOffset = setTimezoneOffset;
			function getTimezoneOffset() {
				return timezoneOffset;
			}
			_Time.getTimezoneOffset = getTimezoneOffset;
			function getDateNumber(date = /* @__PURE__ */ new Date(), offset) {
				if (typeof date === "number") date = new Date(date);
				if (offset === void 0) offset = timezoneOffset;
				return Math.floor((date.valueOf() / minute - offset) / 1440);
			}
			_Time.getDateNumber = getDateNumber;
			function fromDateNumber(value, offset) {
				const date = new Date(value * day);
				if (offset === void 0) offset = timezoneOffset;
				return new Date(+date + offset * minute);
			}
			_Time.fromDateNumber = fromDateNumber;
			const numeric = /\d+(?:\.\d+)?/.source;
			const timeRegExp = new RegExp(`^${[
				"w(?:eek(?:s)?)?",
				"d(?:ay(?:s)?)?",
				"h(?:our(?:s)?)?",
				"m(?:in(?:ute)?(?:s)?)?",
				"s(?:ec(?:ond)?(?:s)?)?"
			].map((unit) => `(${numeric}${unit})?`).join("")}$`);
			function parseTime(source) {
				const capture = timeRegExp.exec(source);
				if (!capture) return 0;
				return (parseFloat(capture[1]) * week || 0) + (parseFloat(capture[2]) * day || 0) + (parseFloat(capture[3]) * hour || 0) + (parseFloat(capture[4]) * minute || 0) + (parseFloat(capture[5]) * second || 0);
			}
			_Time.parseTime = parseTime;
			function parseDate(date) {
				const parsed = parseTime(date);
				if (parsed) date = Date.now() + parsed;
				else if (/^\d{1,2}(:\d{1,2}){1,2}$/.test(date)) date = `${(/* @__PURE__ */ new Date()).toLocaleDateString()}-${date}`;
				else if (/^\d{1,2}-\d{1,2}-\d{1,2}(:\d{1,2}){1,2}$/.test(date)) date = `${(/* @__PURE__ */ new Date()).getFullYear()}-${date}`;
				return date ? new Date(date) : /* @__PURE__ */ new Date();
			}
			_Time.parseDate = parseDate;
			function format(ms) {
				const abs = Math.abs(ms);
				if (abs >= day - hour / 2) return Math.round(ms / day) + "d";
				else if (abs >= hour - minute / 2) return Math.round(ms / hour) + "h";
				else if (abs >= minute - second / 2) return Math.round(ms / minute) + "m";
				else if (abs >= second) return Math.round(ms / second) + "s";
				return ms + "ms";
			}
			_Time.format = format;
			function toDigits(source, length = 2) {
				return source.toString().padStart(length, "0");
			}
			_Time.toDigits = toDigits;
			function template(template, time = /* @__PURE__ */ new Date()) {
				return template.replace("yyyy", time.getFullYear().toString()).replace("yy", time.getFullYear().toString().slice(2)).replace("MM", toDigits(time.getMonth() + 1)).replace("dd", toDigits(time.getDate())).replace("hh", toDigits(time.getHours())).replace("mm", toDigits(time.getMinutes())).replace("ss", toDigits(time.getSeconds())).replace("SSS", toDigits(time.getMilliseconds(), 3));
			}
			_Time.template = template;
		})(Time || (Time = {}));
		//#endregion
		//#region ../../../vendor/schemastery/src/index.ts
		const kSchema = Symbol.for("schemastery");
		const kValidationError = Symbol.for("ValidationError");
		globalThis.__schemastery_index__ ??= 0;
		globalThis.__schemastery_refs__ = void 0;
		var ValidationError = class extends TypeError {
			options;
			name = "ValidationError";
			constructor(message, options) {
				let prefix = "$";
				for (const segment of options.path || []) if (typeof segment === "string") prefix += "." + segment;
				else if (typeof segment === "number") prefix += "[" + segment + "]";
				else if (typeof segment === "symbol") prefix += `[Symbol(${segment.toString()})]`;
				if (prefix.startsWith(".")) prefix = prefix.slice(1);
				super((prefix === "$" ? "" : `${prefix} `) + message);
				this.options = options;
			}
			static is(error) {
				return !!error?.[kValidationError];
			}
		};
		Object.defineProperty(ValidationError.prototype, kValidationError, { value: true });
		const Schema = function(options) {
			const schema = function(data, options = {}) {
				return Schema.resolve(data, schema, options)[0];
			};
			if (options.refs) {
				const refs = mapValues(options.refs, (options) => new Schema(options));
				const getRef = (uid) => refs[uid];
				for (const key in refs) {
					const options = refs[key];
					options.sKey = getRef(options.sKey);
					options.inner = getRef(options.inner);
					options.list = options.list && options.list.map(getRef);
					options.dict = options.dict && mapValues(options.dict, getRef);
				}
				return refs[options.uid];
			}
			Object.assign(schema, options);
			if (typeof schema.callback === "string") try {
				schema.callback = new Function("return " + schema.callback)();
			} catch {}
			Object.defineProperty(schema, "uid", { value: globalThis.__schemastery_index__++ });
			Object.setPrototypeOf(schema, Schema.prototype);
			schema.meta ||= {};
			schema.toString = schema.toString.bind(schema);
			return schema;
		};
		Schema.prototype = Object.create(Function.prototype);
		Schema.prototype[kSchema] = true;
		Object.defineProperty(Schema.prototype, "~standard", { get() {
			return {
				version: 1,
				vendor: "schemastery",
				validate: (value) => {
					try {
						return { value: Schema.resolve(value, this, {})[0] };
					} catch (error) {
						if (ValidationError.is(error)) return { issues: [{
							message: error.message,
							path: error.options.path
						}] };
						throw error;
					}
				}
			};
		} });
		Schema.ValidationError = ValidationError;
		Schema.prototype.toJSON = function toJSON() {
			if (globalThis.__schemastery_refs__) {
				globalThis.__schemastery_refs__[this.uid] ??= JSON.parse(JSON.stringify({ ...this }));
				return this.uid;
			}
			globalThis.__schemastery_refs__ = { [this.uid]: { ...this } };
			globalThis.__schemastery_refs__[this.uid] = JSON.parse(JSON.stringify({ ...this }));
			const result = {
				uid: this.uid,
				refs: globalThis.__schemastery_refs__
			};
			globalThis.__schemastery_refs__ = void 0;
			return result;
		};
		Schema.prototype.set = function set(key, value) {
			this.dict[key] = value;
			return this;
		};
		Schema.prototype.push = function push(value) {
			this.list.push(value);
			return this;
		};
		function mergeDesc(original, messages) {
			const result = typeof original === "string" ? { "": original } : { ...original };
			for (const locale in messages) {
				const value = messages[locale];
				if (value?.$description || value?.$desc) result[locale] = value.$description || value.$desc;
				else if (typeof value === "string") result[locale] = value;
			}
			return result;
		}
		function getInner(value) {
			return value?.$value ?? value?.$inner;
		}
		function extractKeys(data) {
			return filterKeys(data ?? {}, (key) => !key.startsWith("$"));
		}
		Schema.prototype.i18n = function i18n(messages) {
			const schema = Schema(this);
			const desc = mergeDesc(schema.meta.description, messages);
			if (Object.keys(desc).length) schema.meta.description = desc;
			if (schema.dict) schema.dict = mapValues(schema.dict, (inner, key) => {
				return inner.i18n(mapValues(messages, (data) => getInner(data)?.[key] ?? data?.[key]));
			});
			if (schema.list) schema.list = schema.list.map((inner, index) => {
				return inner.i18n(mapValues(messages, (data = {}) => {
					if (Array.isArray(getInner(data))) return getInner(data)[index];
					if (Array.isArray(data)) return data[index];
					return extractKeys(data);
				}));
			});
			if (schema.inner) schema.inner = schema.inner.i18n(mapValues(messages, (data) => {
				if (getInner(data)) return getInner(data);
				return extractKeys(data);
			}));
			if (schema.sKey) schema.sKey = schema.sKey.i18n(mapValues(messages, (data) => data?.$key));
			return schema;
		};
		Schema.prototype.extra = function extra(key, value) {
			const schema = Schema(this);
			schema.meta = {
				...schema.meta,
				[key]: value
			};
			return schema;
		};
		for (const key of [
			"required",
			"disabled",
			"collapse",
			"hidden",
			"loose"
		]) Object.assign(Schema.prototype, { [key](value = true) {
			const schema = Schema(this);
			schema.meta = {
				...schema.meta,
				[key]: value
			};
			return schema;
		} });
		Schema.prototype.deprecated = function deprecated() {
			const schema = Schema(this);
			schema.meta.badges ||= [];
			schema.meta.badges.push({
				text: "deprecated",
				type: "danger"
			});
			return schema;
		};
		Schema.prototype.experimental = function experimental() {
			const schema = Schema(this);
			schema.meta.badges ||= [];
			schema.meta.badges.push({
				text: "experimental",
				type: "warning"
			});
			return schema;
		};
		Schema.prototype.pattern = function pattern(regexp) {
			const schema = Schema(this);
			const pattern = pick(regexp, ["source", "flags"]);
			schema.meta = {
				...schema.meta,
				pattern
			};
			return schema;
		};
		Schema.prototype.simplify = function simplify(value) {
			if (isVolatile(value)) value = value.get();
			if (deepEqual(value, this.meta.default, this.type === "dict")) return null;
			if (isNullable(value)) return value;
			if (this.type === "object" || this.type === "dict") {
				const result = {};
				for (const key in value) {
					const item = (this.type === "object" ? this.dict[key] : this.inner)?.simplify(value[key]);
					if (this.type === "dict" || !isNullable(item)) result[key] = item;
				}
				if (deepEqual(result, this.meta.default, this.type === "dict")) return null;
				return result;
			} else if (this.type === "array" || this.type === "tuple") {
				const result = [];
				value.forEach((value, index) => {
					const schema = this.type === "array" ? this.inner : this.list[index];
					const item = schema ? schema.simplify(value) : value;
					result.push(item);
				});
				return result;
			} else if (this.type === "intersect") {
				const result = {};
				for (const item of this.list) Object.assign(result, item.simplify(value));
				return result;
			} else if (this.type === "union") for (const schema of this.list) try {
				Schema.resolve(value, schema, {});
				return schema.simplify(value);
			} catch {}
			return value;
		};
		Schema.prototype.toString = function toString(inline) {
			return formatters[this.type]?.(this, inline) ?? `Schema<${this.type}>`;
		};
		Schema.prototype.role = function role(role, extra) {
			const schema = Schema(this);
			schema.meta = {
				...schema.meta,
				role,
				extra
			};
			return schema;
		};
		for (const key of [
			"default",
			"link",
			"comment",
			"description",
			"max",
			"min",
			"step"
		]) Object.assign(Schema.prototype, { [key](value) {
			const schema = Schema(this);
			schema.meta = {
				...schema.meta,
				[key]: value
			};
			return schema;
		} });
		Schema.prototype.volatile = function volatile() {
			if (this.meta.volatile) throw new TypeError("volatile schema is already wrapped");
			return this.extra("volatile", true);
		};
		const resolvers = {};
		const checkedVolatile = Symbol("checked-volatile-schema");
		function validateVolatileSchema(schema, path = [], blocked = false, seen = /* @__PURE__ */ new Map()) {
			const states = seen.get(schema) ?? /* @__PURE__ */ new Set();
			if (states.has(blocked)) return;
			states.add(blocked);
			seen.set(schema, states);
			if (schema.meta?.volatile && blocked) throw new ValidationError("volatile fields require a fixed object path without an enclosing volatile field", { path });
			const nested = blocked || !!schema.meta?.volatile;
			if (schema.dict) for (const [key, child] of Object.entries(schema.dict)) validateVolatileSchema(child, [...path, key], nested, seen);
			if (schema.sKey) validateVolatileSchema(schema.sKey, [...path, "<key>"], true, seen);
			if (schema.inner && (schema.type !== "lazy" || schema.inner[kSchema])) validateVolatileSchema(schema.inner, [...path, "*"], true, seen);
			if (schema.list) for (let index = 0; index < schema.list.length; index++) validateVolatileSchema(schema.list[index], [...path, String(index)], true, seen);
		}
		Schema.extend = function extend(type, resolve) {
			resolvers[type] = resolve;
		};
		Schema.resolve = function resolve(data, schema, options = {}, strict = false) {
			if (!schema) return [data];
			if (!options[checkedVolatile]) {
				validateVolatileSchema(schema, options.path);
				options = {
					...options,
					[checkedVolatile]: true
				};
			}
			if (schema.meta?.volatile) {
				const inner = Schema(schema);
				inner.meta = {
					...schema.meta,
					volatile: false
				};
				const [value, adapted] = Schema.resolve(data, inner, options, strict);
				try {
					return [createVolatile(value), adapted];
				} catch (error) {
					throw new ValidationError(error instanceof Error ? error.message : String(error), options);
				}
			}
			if (options.ignore?.(data, schema)) return [data];
			if (isNullable(data) && schema.type !== "lazy") {
				if (schema.meta.required) throw new ValidationError(`missing required value`, options);
				let current = schema;
				let fallback = schema.meta.default;
				while (current?.type === "intersect" && isNullable(fallback)) {
					current = current.list[0];
					fallback = current?.meta.default;
				}
				if (isNullable(fallback)) return [data];
				data = clone(fallback);
			}
			const callback = resolvers[schema.type];
			if (!callback) throw new ValidationError(`unsupported type "${schema.type}"`, options);
			try {
				return callback(data, schema, options, strict);
			} catch (error) {
				if (!schema.meta.loose) throw error;
				return [schema.meta.default];
			}
		};
		Schema.from = function from(source) {
			if (isNullable(source)) return Schema.any();
			else if ([
				"string",
				"number",
				"boolean"
			].includes(typeof source)) return Schema.const(source).required();
			else if (source[kSchema]) return source;
			else if (typeof source === "function") switch (source) {
				case String: return Schema.string().required();
				case Number: return Schema.number().required();
				case Boolean: return Schema.boolean().required();
				case Function: return Schema.function().required();
				default: return Schema.is(source).required();
			}
			else throw new TypeError(`cannot infer schema from ${source}`);
		};
		Schema.lazy = function lazy(builder) {
			const toJSON = () => {
				if (!schema.inner[kSchema]) {
					schema.inner = schema.builder();
					schema.inner.meta = {
						...schema.meta,
						...schema.inner.meta
					};
				}
				return schema.inner.toJSON();
			};
			const schema = new Schema({
				type: "lazy",
				builder,
				inner: { toJSON }
			});
			return schema;
		};
		Schema.natural = function natural() {
			return Schema.number().step(1).min(0);
		};
		Schema.percent = function percent() {
			return Schema.number().step(.01).min(0).max(1).role("slider");
		};
		Schema.date = function date() {
			return Schema.union([Schema.is(Date), Schema.transform(Schema.string().role("datetime"), (value, options) => {
				const date = new Date(value);
				if (isNaN(+date)) throw new ValidationError(`invalid date "${value}"`, options);
				return date;
			}, true)]);
		};
		Schema.regExp = function regExp(flag = "") {
			return Schema.union([Schema.is(RegExp), Schema.transform(Schema.string().role("regexp", { flag }), (value, options) => {
				try {
					return new RegExp(value, flag);
				} catch (e) {
					throw new ValidationError(e.message, options);
				}
			}, true)]);
		};
		Schema.arrayBuffer = function arrayBuffer(encoding) {
			return Schema.union([
				Schema.is(ArrayBuffer),
				Schema.is(SharedArrayBuffer),
				Schema.transform(Schema.any(), (value, options) => {
					if (Binary.isSource(value)) return Binary.fromSource(value);
					throw new ValidationError(`expected ArrayBufferSource but got ${value}`, options);
				}, true),
				...encoding ? [Schema.transform(Schema.string(), (value, options) => {
					try {
						return encoding === "base64" ? Binary.fromBase64(value) : Binary.fromHex(value);
					} catch (e) {
						throw new ValidationError(e.message, options);
					}
				}, true)] : []
			]);
		};
		Schema.extend("lazy", (data, schema, options, strict) => {
			if (!schema.inner[kSchema]) {
				schema.inner = schema.builder();
				schema.inner.meta = {
					...schema.meta,
					...schema.inner.meta
				};
				validateVolatileSchema(schema.inner, options.path, true);
			}
			return Schema.resolve(data, schema.inner, options, strict);
		});
		Schema.extend("any", (data) => {
			return [data];
		});
		Schema.extend("never", (data, _, options) => {
			throw new ValidationError(`expected nullable but got ${data}`, options);
		});
		Schema.extend("const", (data, { value }, options) => {
			if (deepEqual(data, value)) return [value];
			throw new ValidationError(`expected ${value} but got ${data}`, options);
		});
		function checkWithinRange(data, meta, description, options, skipMin = false) {
			const { max = Infinity, min = -Infinity } = meta;
			if (data > max) throw new ValidationError(`expected ${description} <= ${max} but got ${data}`, options);
			if (data < min && !skipMin) throw new ValidationError(`expected ${description} >= ${min} but got ${data}`, options);
		}
		Schema.extend("string", (data, { meta }, options) => {
			if (typeof data !== "string") throw new ValidationError(`expected string but got ${data}`, options);
			if (meta.pattern) {
				const regexp = new RegExp(meta.pattern.source, meta.pattern.flags);
				if (!regexp.test(data)) throw new ValidationError(`expect string to match regexp ${regexp}`, options);
			}
			checkWithinRange(data.length, meta, "string length", options);
			return [data];
		});
		function decimalShift(data, digits) {
			const str = data.toString();
			if (str.includes("e")) return data * Math.pow(10, digits);
			const index = str.indexOf(".");
			if (index === -1) return data * Math.pow(10, digits);
			const frac = str.slice(index + 1);
			const integer = str.slice(0, index);
			if (frac.length <= digits) return +(integer + frac.padEnd(digits, "0"));
			return +(integer + frac.slice(0, digits) + "." + frac.slice(digits));
		}
		function isMultipleOf(data, min, step) {
			step = Math.abs(step);
			if (!/^\d+\.\d+$/.test(step.toString())) return (data - min) % step === 0;
			const index = step.toString().indexOf(".");
			const digits = step.toString().slice(index + 1).length;
			return Math.abs(decimalShift(data, digits) - decimalShift(min, digits)) % decimalShift(step, digits) === 0;
		}
		Schema.extend("number", (data, { meta }, options) => {
			if (typeof data !== "number") throw new ValidationError(`expected number but got ${data}`, options);
			checkWithinRange(data, meta, "number", options);
			const { step } = meta;
			if (step && !isMultipleOf(data, meta.min ?? 0, step)) throw new ValidationError(`expected number multiple of ${step} but got ${data}`, options);
			return [data];
		});
		Schema.extend("boolean", (data, _, options) => {
			if (typeof data === "boolean") return [data];
			throw new ValidationError(`expected boolean but got ${data}`, options);
		});
		Schema.extend("bitset", (data, { bits, meta }, options) => {
			let value = 0, keys = [];
			if (typeof data === "number") {
				value = data;
				for (const key in bits) if (data & bits[key]) keys.push(key);
			} else if (Array.isArray(data)) {
				keys = data;
				for (const key of keys) {
					if (typeof key !== "string") throw new ValidationError(`expected string but got ${key}`, options);
					if (key in bits) value |= bits[key];
				}
			} else throw new ValidationError(`expected number or array but got ${data}`, options);
			if (value === meta.default) return [value];
			return [value, keys];
		});
		Schema.extend("function", (data, _, options) => {
			if (typeof data === "function") return [data];
			throw new ValidationError(`expected function but got ${data}`, options);
		});
		Schema.extend("is", (data, { constructor }, options) => {
			if (typeof constructor === "function") {
				if (data instanceof constructor) return [data];
				throw new ValidationError(`expected ${constructor.name} but got ${data}`, options);
			} else {
				if (isNullable(data)) throw new ValidationError(`expected ${constructor} but got ${data}`, options);
				let prototype = Object.getPrototypeOf(data);
				while (prototype) {
					if (prototype.constructor?.name === constructor) return [data];
					prototype = Object.getPrototypeOf(prototype);
				}
				throw new ValidationError(`expected ${constructor} but got ${data}`, options);
			}
		});
		function property(data, key, schema, options) {
			try {
				const [value, adapted] = Schema.resolve(data[key], schema, {
					...options,
					path: [...options.path || [], key]
				});
				if (adapted !== void 0) data[key] = adapted;
				return value;
			} catch (e) {
				if (!options?.autofix) throw e;
				delete data[key];
				return schema.meta.volatile ? createVolatile(schema.meta.default) : schema.meta.default;
			}
		}
		Schema.extend("array", (data, { inner, meta }, options) => {
			if (!Array.isArray(data)) throw new ValidationError(`expected array but got ${data}`, options);
			checkWithinRange(data.length, meta, "array length", options, !isNullable(inner.meta.default));
			return [data.map((_, index) => property(data, index, inner, options))];
		});
		Schema.extend("dict", (data, { inner, sKey }, options, strict) => {
			if (!isPlainObject(data)) throw new ValidationError(`expected object but got ${data}`, options);
			const result = {};
			for (const key in data) {
				let rKey;
				try {
					rKey = Schema.resolve(key, sKey, options)[0];
				} catch (error) {
					if (strict) continue;
					throw error;
				}
				result[rKey] = property(data, key, inner, options);
				data[rKey] = data[key];
				if (key !== rKey) delete data[key];
			}
			return [result];
		});
		Schema.extend("tuple", (data, { list }, options, strict) => {
			if (!Array.isArray(data)) throw new ValidationError(`expected array but got ${data}`, options);
			const result = list.map((inner, index) => property(data, index, inner, options));
			if (strict) return [result];
			result.push(...data.slice(list.length));
			return [result];
		});
		function merge(result, data) {
			for (const key in data) {
				if (key in result) continue;
				result[key] = data[key];
			}
		}
		Schema.extend("object", (data, { dict }, options, strict) => {
			if (!isPlainObject(data)) throw new ValidationError(`expected object but got ${data}`, options);
			const result = {};
			for (const key in dict) {
				const value = property(data, key, dict[key], options);
				if (!isNullable(value) || key in data) result[key] = value;
			}
			if (!strict) merge(result, data);
			return [result];
		});
		Schema.extend("union", (data, { list, toString }, options, strict) => {
			const messages = [];
			for (const inner of list) try {
				return Schema.resolve(data, inner, options, strict);
			} catch (error) {
				messages.push(error);
			}
			throw new ValidationError(`expected ${toString()} but got ${JSON.stringify(data)}`, options);
		});
		Schema.extend("intersect", (data, { list, toString }, options, strict) => {
			if (!list.length) return [data];
			let result;
			for (const inner of list) {
				const value = Schema.resolve(data, inner, options, true)[0];
				if (isNullable(value)) continue;
				if (isNullable(result)) result = value;
				else if (typeof result !== typeof value) throw new ValidationError(`expected ${toString()} but got ${JSON.stringify(data)}`, options);
				else if (typeof value === "object") merge(result ??= {}, value);
				else if (result !== value) throw new ValidationError(`expected ${toString()} but got ${JSON.stringify(data)}`, options);
			}
			if (!strict && isPlainObject(data)) merge(result, data);
			return [result];
		});
		Schema.extend("transform", (data, { inner, callback, preserve }, options) => {
			const [result, adapted = data] = Schema.resolve(data, inner, options, true);
			if (preserve) return [callback(result)];
			else return [callback(result), callback(adapted)];
		});
		const formatters = {};
		function defineMethod(name, keys, format) {
			formatters[name] = format;
			Object.assign(Schema, { [name](...args) {
				const schema = new Schema({ type: name });
				keys.forEach((key, index) => {
					switch (key) {
						case "sKey":
							schema.sKey = args[index] ?? Schema.string();
							break;
						case "inner":
							schema.inner = Schema.from(args[index]);
							break;
						case "list":
							schema.list = args[index].map(Schema.from);
							break;
						case "dict":
							schema.dict = mapValues(args[index], Schema.from);
							break;
						case "bits":
							schema.bits = {};
							for (const key in args[index]) {
								if (typeof args[index][key] !== "number") continue;
								schema.bits[key] = args[index][key];
							}
							break;
						case "callback": {
							const callback = schema.callback = args[index];
							callback["toJSON"] ||= () => callback.toString();
							break;
						}
						case "constructor": {
							const constructor = schema.constructor = args[index];
							if (typeof constructor === "function") constructor["toJSON"] ||= () => constructor["name"];
							break;
						}
						default: schema[key] = args[index];
					}
				});
				if (name === "object" || name === "dict") schema.meta.default = {};
				else if (name === "array" || name === "tuple") schema.meta.default = [];
				else if (name === "bitset") schema.meta.default = 0;
				return schema;
			} });
		}
		defineMethod("is", ["constructor"], ({ constructor }) => {
			if (typeof constructor === "function") return constructor.name;
			else return constructor;
		});
		defineMethod("any", [], () => "any");
		defineMethod("never", [], () => "never");
		defineMethod("const", ["value"], ({ value }) => typeof value === "string" ? JSON.stringify(value) : value);
		defineMethod("string", [], () => "string");
		defineMethod("number", [], () => "number");
		defineMethod("boolean", [], () => "boolean");
		defineMethod("bitset", ["bits"], () => "bitset");
		defineMethod("function", [], () => "function");
		defineMethod("array", ["inner"], ({ inner }) => `${inner.toString(true)}[]`);
		defineMethod("dict", ["inner", "sKey"], ({ inner, sKey }) => `{ [key: ${sKey.toString()}]: ${inner.toString()} }`);
		defineMethod("tuple", ["list"], ({ list }) => `[${list.map((inner) => inner.toString()).join(", ")}]`);
		defineMethod("object", ["dict"], ({ dict }) => {
			if (Object.keys(dict).length === 0) return "{}";
			return `{ ${Object.entries(dict).map(([key, inner]) => {
				return `${key}${inner.meta.required ? "" : "?"}: ${inner.toString()}`;
			}).join(", ")} }`;
		});
		defineMethod("union", ["list"], ({ list }, inline) => {
			const result = list.map(({ toString: format }) => format()).join(" | ");
			return inline ? `(${result})` : result;
		});
		defineMethod("intersect", ["list"], ({ list }) => {
			return `${list.map((inner) => inner.toString(true)).join(" & ")}`;
		});
		defineMethod("transform", [
			"inner",
			"callback",
			"preserve"
		], ({ inner }, isInner) => inner.toString(isInner));
		//#endregion
		//#region lib/types/client/target-key.js
		/**
		* Compare captured JSON targets independently of object key insertion order.
		* @param target - captured immutable target.
		* @returns canonical JSON identity.
		*/
		function menuTargetKey(target) {
			return JSON.stringify(target, (_key, value) => {
				if (typeof value !== "object" || value === null || Array.isArray(value)) return value;
				return Object.fromEntries(Object.entries(value).sort(([left], [right]) => left.localeCompare(right)));
			});
		}
		//#endregion
		//#region lib/types/client/registry.js
		/** React-free registry; each occurrence captures the providers registered at opening. */
		var MenuRegistry = class {
			failed;
			timeoutMs;
			providers = /* @__PURE__ */ new Map();
			occurrences = /* @__PURE__ */ new Set();
			disposed = false;
			constructor(failed, timeoutMs) {
				this.failed = failed;
				this.timeoutMs = timeoutMs;
			}
			/**
			* Register actions for subsequent openings.
			* @param input - provider definition.
			* @returns removal of this contribution and its pending resolutions.
			*/
			register(input) {
				if (this.disposed) throw new Error("Menu registry is disposed");
				const key = `${encodeURIComponent(input.target)}/${encodeURIComponent(input.id)}`;
				if (this.providers.has(key)) throw new Error(`Duplicate menu contribution: ${key}`);
				const provider = {
					id: input.id,
					kind: input.target,
					order: input.order,
					lifetime: new AbortController(),
					observe: (target) => input.observe?.(target.value) ?? [],
					resolve: (target, signal) => input.resolve(target.value, signal)
				};
				this.providers.set(key, provider);
				return () => {
					if (provider.lifetime.signal.aborted) return;
					this.providers.delete(key);
					provider.lifetime.abort();
				};
			}
			/**
			* Resolve an independently owned occurrence.
			* @param target - captured clicked identity.
			* @returns presentation, selection, and disposal.
			*/
			resolve(target) {
				if (this.disposed) throw new Error("Menu registry is disposed");
				const lifetime = new AbortController();
				const source = (0, _deepseek_ai_dsh_client_store.createSnapshotStore)([]);
				const resolved = /* @__PURE__ */ new Map();
				const callbacks = /* @__PURE__ */ new Map();
				const providers = [...this.providers.values()].filter((p) => p.kind === target.kind).sort((a, b) => a.order - b.order || a.id.localeCompare(b.id));
				const refresh = () => {
					source.set(providers.flatMap((provider) => provider.lifetime.signal.aborted ? [] : resolved.get(provider) ?? []));
				};
				const cleanup = [];
				const dispose = () => {
					if (lifetime.signal.aborted) return;
					lifetime.abort();
					for (const detach of cleanup) detach();
					callbacks.clear();
					this.occurrences.delete(dispose);
				};
				this.occurrences.add(dispose);
				const pending = [];
				for (const provider of providers) {
					let preparation;
					let actionIds = [];
					let preparing;
					const clear = () => {
						for (const id of actionIds) callbacks.delete(id);
						actionIds = [];
						resolved.delete(provider);
						refresh();
					};
					const prepare = () => {
						preparation?.abort();
						const deadline = new AbortController();
						preparation = deadline;
						const signal = AbortSignal.any([
							lifetime.signal,
							provider.lifetime.signal,
							deadline.signal
						]);
						const accept = (actions) => {
							if (signal.aborted) return;
							const next = /* @__PURE__ */ new Map();
							const project = (action, parent) => {
								const id = `${parent}/${encodeURIComponent(action.id)}`;
								if (next.has(id)) throw new Error(`Duplicate menu action: ${id}`);
								next.set(id, action);
								const { run: _run, failureLabel: _failureLabel, children, ...view } = action;
								return {
									...view,
									id,
									...children === void 0 ? {} : { children: children.map((child) => project(child, id)) }
								};
							};
							const views = actions.map((action) => project(action, encodeURIComponent(provider.id)));
							for (const id of actionIds) callbacks.delete(id);
							for (const [id, action] of next) callbacks.set(id, {
								provider,
								action
							});
							actionIds = [...next.keys()];
							resolved.set(provider, views);
							refresh();
						};
						preparing = void 0;
						try {
							const result = provider.resolve(target, signal);
							if (result instanceof Promise) clear();
							preparing = result instanceof Promise ? new Promise((settle) => {
								const timer = this.timeoutMs === void 0 ? void 0 : setTimeout(() => {
									deadline.abort();
									this.failed();
								}, this.timeoutMs);
								const finish = () => {
									clearTimeout(timer);
									signal.removeEventListener("abort", finish);
									settle();
								};
								signal.addEventListener("abort", finish, { once: true });
								result.then(accept).catch(() => {
									if (!signal.aborted) this.failed();
								}).finally(finish);
							}) : void 0;
							if (!(result instanceof Promise)) accept(result);
						} catch (_error) {
							if (!signal.aborted) {
								clear();
								this.failed();
							}
						}
					};
					const subscriptions = [];
					const withdraw = () => {
						preparation?.abort();
						for (const detach of subscriptions.splice(0)) detach();
						clear();
					};
					provider.lifetime.signal.addEventListener("abort", withdraw, { once: true });
					cleanup.push(() => {
						provider.lifetime.signal.removeEventListener("abort", withdraw);
						withdraw();
					});
					try {
						for (const input of provider.observe(target)) subscriptions.push(input.subscribe(prepare));
						prepare();
					} catch (_error) {
						if (!lifetime.signal.aborted && !provider.lifetime.signal.aborted) this.failed();
					}
					pending.push((async () => {
						let current;
						do {
							current = preparing;
							await current;
						} while (current !== preparing);
					})());
				}
				return {
					source,
					dispose,
					ready: Promise.all(pending).then(() => {}),
					select: (id) => {
						const selected = callbacks.get(id);
						if (lifetime.signal.aborted || selected === void 0 || selected.provider.lifetime.signal.aborted || selected.action.disabled || selected.action.children !== void 0) return;
						const run = selected.action.run;
						dispose();
						try {
							Promise.resolve(run?.()).catch(() => {
								this.failed(selected.action.failureLabel);
							});
						} catch (_error) {
							this.failed(selected.action.failureLabel);
						}
					}
				};
			}
			/** Cancel every occurrence and reject further registrations or resolutions. */
			dispose() {
				if (this.disposed) return;
				this.disposed = true;
				for (const dispose of this.occurrences) dispose();
				for (const provider of this.providers.values()) provider.lifetime.abort();
				this.providers.clear();
			}
		};
		//#endregion
		//#region lib/types/client/store.js
		/** Single-popup viewing state and feedback retained outside trigger components. */
		/**
		* Create the root popup store.
		* @returns the presentation and its mutation actions.
		*/
		function createMenuStore() {
			return (0, _deepseek_ai_dsh_client_store.defineStore)({
				init: () => ({
					occurrence: 0,
					closing: false,
					replacing: false,
					target: null,
					point: {
						x: 0,
						y: 0
					},
					items: [],
					notice: null
				}),
				actions: {
					open(draft, target, point) {
						draft.occurrence++;
						draft.closing = false;
						draft.replacing = false;
						draft.target = target;
						draft.point = point;
						draft.items = [];
					},
					items(draft, items) {
						draft.items = items;
					},
					exit(draft, replacing) {
						draft.closing = true;
						draft.replacing = replacing;
					},
					close(draft) {
						draft.target = null;
						draft.items = [];
					},
					notify(draft, notice) {
						draft.notice = notice;
					}
				}
			});
		}
		//#endregion
		//#region lib/types/client/locales.js
		/** Context-menu operation feedback, separate from plugin display metadata. */
		const dictionaries = {
			zh: { failed: "操作失败，请重试" },
			en: { failed: "Could not complete the action. Try again." },
			ja: { failed: "操作に失敗しました。再試行してください。" },
			es: { failed: "No se pudo completar la acción. Inténtelo de nuevo." },
			fr: { failed: "Impossible de terminer l’action. Réessayez." },
			de: { failed: "Die Aktion konnte nicht abgeschlossen werden. Versuchen Sie es erneut." },
			ru: { failed: "Не удалось выполнить действие. Повторите попытку." },
			pt: { failed: "Não foi possível concluir a ação. Tente novamente." }
		};
		//#endregion
		//#region lib/types/client/View.js
		/** Shared Web renderer for Client menu contributions. */
		/**
		* @param actions - plain presentation.
		* @param catalog - effective user shortcuts.
		* @param renderSlot - owner-authorized keyed icon renderer.
		* @returns Existing Menu entries.
		*/
		function menuEntries(actions, catalog, renderSlot) {
			return actions.flatMap((action) => {
				const shortcut = catalog.find((entry) => entry.id === action.commandId);
				const item = {
					id: action.id,
					label: action.label,
					...shortcut === void 0 ? {} : { shortcut },
					...action.disabled === void 0 ? {} : { disabled: action.disabled },
					...action.danger === void 0 ? {} : { danger: action.danger },
					...action.icon === void 0 ? {} : { icon: renderSlot("contextMenu.icon", { icon: action.icon }, { entryKey: action.icon.key }) },
					...action.children === void 0 ? {} : { submenu: menuEntries(action.children, catalog, renderSlot) }
				};
				return action.separatorBefore ? [{
					type: "separator",
					id: `${action.id}:separator`
				}, item] : [item];
			});
		}
		/** @param props - registered state, target-specific legacy rows, and actions. @returns one menu and persistent feedback. */
		function MenuOverlay({ useStore, actions, renderSlot, close, select, returnFocus, useCatalog }) {
			const catalog = useCatalog((value) => value);
			const { target, point, items, notice, occurrence, closing, replacing } = useStore((s) => s);
			return (0, react_jsx_runtime.jsxs)(react_jsx_runtime.Fragment, { children: [(0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Menu, {
				open: target !== null,
				anchor: null,
				portal: true,
				autoFocus: true,
				contextual: true,
				returnFocus,
				exiting: closing ? replacing ? "replace" : "dismiss" : void 0,
				getAnchorRect: () => new DOMRect(point.x, point.y, 0, 0),
				items: menuEntries(items.filter((item) => !item.afterContent), catalog, renderSlot),
				footer: menuEntries(items.filter((item) => item.afterContent), catalog, renderSlot),
				onClose: close,
				onSelect: select,
				children: target !== null && renderSlot("contextMenu.content", {
					target,
					dismiss: close
				}, { entryKey: target.kind })
			}, occurrence), notice !== null && (0, react_jsx_runtime.jsx)(_deepseek_ai_dsh_client_ui_primitives.Toast, {
				text: notice.text,
				...notice.action === void 0 ? {} : { actions: [notice.action] },
				...notice.success ? { tone: "success" } : {},
				onDone: () => {
					actions.notify(null);
				}
			}, notice.id)] });
		}
		//#endregion
		//#region lib/types/client/trigger.js
		/**
		* Create stable bindings for the plugin's injected hook.
		* @param open - Open against the supplied captured target and viewport position.
		* @param active - Whether the plugin still owns its service.
		* @returns Ordinary trigger-prop factory; no React subscription or context is created.
		*/
		function createContextMenuBindings(open, active) {
			return { getTriggerProps(target, handlers = {}) {
				if (target === void 0) return handlers;
				const show = (event, x, y) => {
					if (event.defaultPrevented || !active()) return;
					const captured = typeof target === "function" ? target() : target;
					event.preventDefault();
					event.stopPropagation();
					event.currentTarget.focus({ preventScroll: true });
					open(captured, {
						x,
						y
					});
				};
				return {
					...handlers,
					tabIndex: handlers.tabIndex ?? 0,
					onContextMenu(event) {
						handlers.onContextMenu?.(event);
						const rect = event.clientX === 0 && event.clientY === 0 ? event.currentTarget.getBoundingClientRect() : void 0;
						show(event, rect?.left ?? event.clientX, rect?.bottom ?? event.clientY);
					},
					onKeyDown(event) {
						handlers.onKeyDown?.(event);
						if (event.defaultPrevented || event.repeat || event.nativeEvent.isComposing || event.ctrlKey || event.altKey || event.metaKey || !(event.key === "ContextMenu" || event.key === "F10" && event.shiftKey)) return;
						const rect = event.currentTarget.getBoundingClientRect();
						show(event, rect.left, rect.bottom);
					}
				};
			} };
		}
		//#endregion
		//#region lib/types/client/index.js
		/** Window-local menu capability and its Web overlay. */
		/** Validated Client menu preparation budget. */
		const Config = Schema.object({ resolutionTimeoutMs: Schema.natural().min(1).default(1e3) });
		/** Required overlay and localization services. */
		const inject = [
			"slots",
			"locale",
			"shortcuts"
		];
		/** @param ctx - plugin-owned Client context. @param config - validated preparation budget. */
		function apply(ctx, config = Config({})) {
			ctx.effect(() => ctx.locale.register("contextMenu", dictionaries), "context menu: locale");
			const t = ctx.locale.bind("contextMenu");
			const handle = createMenuStore();
			const instance = handle.create();
			const store = {
				...handle,
				create: () => instance
			};
			const targetSource = (0, _deepseek_ai_dsh_client_store.createSnapshotStore)(null);
			let sequence = 0;
			let active;
			let closeTimer;
			let openingTimer;
			let finishOpening;
			let returnFocus = null;
			let disposed = false;
			const notify = (text, success = false, action) => {
				if (disposed) return;
				const id = ++sequence;
				instance.actions.notify({
					id,
					text,
					success,
					...action === void 0 ? {} : { action: {
						label: action.label,
						onClick: () => {
							if (disposed || instance.getSnapshot().notice?.id !== id) return;
							instance.actions.notify(null);
							try {
								Promise.resolve(action.run()).catch(() => {
									notify(action.failureLabel);
								});
							} catch (_error) {
								notify(action.failureLabel);
							}
						}
					} }
				});
			};
			const registry = new MenuRegistry((message) => {
				notify(message ?? t("failed"));
			}, config.resolutionTimeoutMs);
			const cancelOpening = () => {
				clearTimeout(openingTimer);
				finishOpening?.();
				finishOpening = void 0;
				active?.unsubscribe();
				active?.resolution.dispose();
				active = void 0;
				targetSource.set(null);
			};
			const dismiss = (replacing) => {
				cancelOpening();
				clearTimeout(closeTimer);
				if (instance.getSnapshot().target === null) return;
				instance.actions.exit(replacing);
				closeTimer = setTimeout(() => {
					instance.actions.close();
				}, replacing ? _deepseek_ai_dsh_client_ui_primitives.MENU_TRANSITION_MS.replace : _deepseek_ai_dsh_client_ui_primitives.MENU_TRANSITION_MS.dismiss);
			};
			const close = () => {
				dismiss(false);
			};
			const contextMenus = {
				active: targetSource,
				register: (input) => registry.register(input),
				resolve: (target) => registry.resolve(target),
				notify,
				close,
				open: (target, point) => {
					const previous = active;
					if (previous !== void 0 && Math.hypot(previous.point.x - point.x, previous.point.y - point.y) <= 4 && menuTargetKey(previous.target) === menuTargetKey(target)) {
						close();
						return;
					}
					const focused = document.activeElement;
					if (!(focused instanceof Element) || focused.closest("[data-context-menu]") === null) returnFocus = focused instanceof HTMLElement ? focused : null;
					dismiss(true);
					const resolution = registry.resolve(target);
					targetSource.set(target);
					let displayed = false;
					const publish = () => {
						if (displayed) instance.actions.items(resolution.source.getSnapshot());
					};
					const occurrence = {
						resolution,
						unsubscribe: resolution.source.subscribe(publish),
						target,
						point
					};
					active = occurrence;
					const delay = new Promise((resolve) => {
						finishOpening = resolve;
						openingTimer = setTimeout(resolve, _deepseek_ai_dsh_client_ui_primitives.MENU_TRANSITION_MS.prepare);
					});
					Promise.all([resolution.ready, delay]).then(() => {
						if (active !== occurrence) return;
						clearTimeout(closeTimer);
						instance.actions.open(target, point);
						displayed = true;
						publish();
					});
				}
			};
			ctx.provide("contextMenus", contextMenus);
			const bindings = createContextMenuBindings((target, point) => {
				contextMenus.open(target, point);
			}, () => !disposed);
			ctx.slots.provideRoot({ props: { useContextMenu: () => bindings } });
			ctx.effect(() => {
				const outside = (event) => {
					if (event.button === 2 || event.target instanceof Element && event.target.closest("[role=\"menu\"], [data-context-menu-trigger]")) return;
					close();
				};
				const context = (event) => {
					if (!event.defaultPrevented) close();
				};
				const escape = (event) => {
					if (active !== void 0 && instance.getSnapshot().target === null && event.key === "Escape") {
						event.preventDefault();
						event.stopImmediatePropagation();
						close();
					}
				};
				document.addEventListener("pointerdown", outside);
				document.addEventListener("contextmenu", context);
				window.addEventListener("keydown", escape, true);
				return () => {
					disposed = true;
					cancelOpening();
					clearTimeout(closeTimer);
					registry.dispose();
					instance.actions.close();
					document.removeEventListener("pointerdown", outside);
					document.removeEventListener("contextmenu", context);
					window.removeEventListener("keydown", escape, true);
				};
			}, "context menu: lifetime");
			ctx.slots.inject("shell.overlay", () => ctx.slots.register({
				name: "shell.overlay",
				id: "context-menu",
				store,
				children: {
					"contextMenu.content": {
						kind: "keyed",
						scope: "root"
					},
					"contextMenu.icon": {
						kind: "keyed",
						scope: "root",
						inline: true
					}
				},
				inject: () => ({
					hooks: { catalog: ctx.shortcuts.catalog },
					close,
					returnFocus: () => returnFocus,
					select: (id) => {
						const selected = active?.resolution;
						active?.unsubscribe();
						active = void 0;
						targetSource.set(null);
						dismiss(false);
						selected?.select(id);
						selected?.dispose();
					}
				})
			}, MenuOverlay));
		}
		//#endregion
		exports.Config = Config;
		exports.apply = apply;
		exports.createMenuStore = createMenuStore;
		exports.inject = inject;
		return module.exports;
	}
});

//# sourceMappingURL=client.js.map