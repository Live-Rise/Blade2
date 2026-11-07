import { addAbortListener } from "node:events";
import z from "@deepseek-ai/schemastery";
import { Remote, TypertRemoteService } from "@deepseek-ai/dsh-typert-protocol";
//#region lib/types/index.js
/** Trajectory Host inspector retaining tool values only for live Session instances. */
var __runInitializers = function(thisArg, initializers, value) {
	var useValue = arguments.length > 2;
	for (var i = 0; i < initializers.length; i++) value = useValue ? initializers[i].call(thisArg, value) : initializers[i].call(thisArg);
	return useValue ? value : void 0;
};
var __esDecorate = function(ctor, descriptorIn, decorators, contextIn, initializers, extraInitializers) {
	function accept(f) {
		if (f !== void 0 && typeof f !== "function") throw new TypeError("Function expected");
		return f;
	}
	var kind = contextIn.kind, key = kind === "getter" ? "get" : kind === "setter" ? "set" : "value";
	var target = !descriptorIn && ctor ? contextIn["static"] ? ctor : ctor.prototype : null;
	var descriptor = descriptorIn || (target ? Object.getOwnPropertyDescriptor(target, contextIn.name) : {});
	var _, done = false;
	for (var i = decorators.length - 1; i >= 0; i--) {
		var context = {};
		for (var p in contextIn) context[p] = p === "access" ? {} : contextIn[p];
		for (var p in contextIn.access) context.access[p] = contextIn.access[p];
		context.addInitializer = function(f) {
			if (done) throw new TypeError("Cannot add initializers after decoration has completed");
			extraInitializers.push(accept(f || null));
		};
		var result = (0, decorators[i])(kind === "accessor" ? {
			get: descriptor.get,
			set: descriptor.set
		} : descriptor[key], context);
		if (kind === "accessor") {
			if (result === void 0) continue;
			if (result === null || typeof result !== "object") throw new TypeError("Object expected");
			if (_ = accept(result.get)) descriptor.get = _;
			if (_ = accept(result.set)) descriptor.set = _;
			if (_ = accept(result.init)) initializers.unshift(_);
		} else if (_ = accept(result)) if (kind === "field") initializers.unshift(_);
		else descriptor[key] = _;
	}
	if (target) Object.defineProperty(target, contextIn.name, descriptor);
	done = true;
};
var __addDisposableResource = function(env, value, async) {
	if (value !== null && value !== void 0) {
		if (typeof value !== "object" && typeof value !== "function") throw new TypeError("Object expected.");
		var dispose, inner;
		if (async) {
			if (!Symbol.asyncDispose) throw new TypeError("Symbol.asyncDispose is not defined.");
			dispose = value[Symbol.asyncDispose];
		}
		if (dispose === void 0) {
			if (!Symbol.dispose) throw new TypeError("Symbol.dispose is not defined.");
			dispose = value[Symbol.dispose];
			if (async) inner = dispose;
		}
		if (typeof dispose !== "function") throw new TypeError("Object not disposable.");
		if (inner) dispose = function() {
			try {
				inner.call(this);
			} catch (e) {
				return Promise.reject(e);
			}
		};
		env.stack.push({
			value,
			dispose,
			async
		});
	} else if (async) env.stack.push({ async: true });
	return value;
};
var __disposeResources = (function(SuppressedError) {
	return function(env) {
		function fail(e) {
			env.error = env.hasError ? new SuppressedError(e, env.error, "An error was suppressed during disposal.") : e;
			env.hasError = true;
		}
		var r, s = 0;
		function next() {
			while (r = env.stack.pop()) try {
				if (!r.async && s === 1) return s = 0, env.stack.push(r), Promise.resolve().then(next);
				if (r.dispose) {
					var result = r.dispose.call(r.value);
					if (r.async) return s |= 2, Promise.resolve(result).then(next, function(e) {
						fail(e);
						return next();
					});
				} else s |= 1;
			} catch (e) {
				fail(e);
			}
			if (s === 1) return env.hasError ? Promise.reject(env.error) : Promise.resolve();
			if (env.hasError) throw env.error;
		}
		return next();
	};
})(typeof SuppressedError === "function" ? SuppressedError : function(error, suppressed, message) {
	var e = new Error(message);
	return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
});
function emptyResults() {
	return {
		values: /* @__PURE__ */ new Map(),
		listeners: /* @__PURE__ */ new Set()
	};
}
let SessionToolResults = (() => {
	let _classSuper = TypertRemoteService;
	let _instanceExtraInitializers = [];
	let _watch_decorators;
	return class SessionToolResults extends _classSuper {
		static {
			const _metadata = typeof Symbol === "function" && Symbol.metadata ? Object.create(_classSuper[Symbol.metadata] ?? null) : void 0;
			_watch_decorators = [Remote({ mode: "stream" })];
			__esDecorate(this, null, _watch_decorators, {
				kind: "method",
				name: "watch",
				static: false,
				private: false,
				access: {
					has: (obj) => "watch" in obj,
					get: (obj) => obj.watch
				},
				metadata: _metadata
			}, null, _instanceExtraInitializers);
			if (_metadata) Object.defineProperty(this, Symbol.metadata, {
				enumerable: true,
				configurable: true,
				writable: true,
				value: _metadata
			});
		}
		static inject = ["sessions", "typert"];
		static Config = z.object({ maxResultsPerSession: z.natural().min(1).default(1e3) });
		results = (__runInitializers(this, _instanceExtraInitializers), /* @__PURE__ */ new Map());
		lifetime = new AbortController();
		/**
		* @param ctx - Owner of the live Session registry and tool result notifications.
		* @param config - Validated per-Session retention limit.
		*/
		constructor(ctx, config) {
			super(ctx, "sessionToolResults", { namespace: "toolResults" });
			ctx.on("tools/result", (exec, result) => {
				const session = exec.agent?.session;
				if (result.isError || session === void 0 || ctx.sessions.get(session.id) !== session) return;
				const state = this.forSession(session);
				state.values.set(exec.callId, result.value);
				if (state.values.size <= config.maxResultsPerSession) return;
				for (const oldest of state.values.keys()) {
					state.values.delete(oldest);
					if (typeof oldest === "number") for (const listener of state.listeners) listener(oldest);
					break;
				}
			}, { global: true });
			ctx.on("session/event", (session, event) => {
				const callId = event.type === "tool/result" ? event.data.message.toolCallId : event.type === "tool/ptc-dispatch" ? event.data.subCallId : void 0;
				if (callId === void 0) return;
				const state = this.results.get(session);
				const value = state?.values.get(callId);
				if (state === void 0 || value === void 0) return;
				state.values.delete(callId);
				state.values.set(event.seq, value);
				for (const listener of state.listeners) listener(event.seq);
			}, { global: true });
			ctx.on("session/disposed", (session) => {
				const state = this.results.get(session);
				if (state === void 0) return;
				this.results.delete(session);
				state.values.clear();
				for (const listener of state.listeners) listener();
			}, { global: true });
			ctx.effect(() => () => {
				this.lifetime.abort();
				for (const state of this.results.values()) state.values.clear();
				this.results.clear();
			}, "tool-results: live values and followers");
		}
		forSession(session) {
			let state = this.results.get(session);
			if (state === void 0) {
				state = emptyResults();
				this.results.set(session, state);
			}
			return state;
		}
		forRequest(request) {
			const session = this.ctx.sessions.get(request.sessionId);
			return session === void 0 ? emptyResults() : this.forSession(session);
		}
		/**
		* Observe one committed result without loading a Session or reading its history.
		* Values disappear on count eviction or when the exact Session instance leaves
		* the registry. A cold read stays unavailable; opening a new stream observes
		* the current instance. Inspection does not extend a result's retention.
		* @param request - Authorized Session identity and durable result event sequence.
		* @param signal - Remote consumer lifetime.
		* @returns Current availability and changes until cancellation or service disposal.
		*/
		async *watch(request, signal) {
			const env_1 = {
				stack: [],
				error: void 0,
				hasError: false
			};
			try {
				const lifetime = AbortSignal.any([signal, this.lifetime.signal]);
				if (lifetime.aborted) return;
				const state = this.forRequest(request);
				let update = Promise.withResolvers();
				const changed = (seq) => {
					if (seq === void 0 || seq === request.seq) update.resolve();
				};
				state.listeners.add(changed);
				__addDisposableResource(env_1, addAbortListener(lifetime, () => {
					update.resolve();
				}), false);
				try {
					while (!lifetime.aborted) {
						const value = state.values.get(request.seq);
						yield value === void 0 ? { available: false } : {
							available: true,
							value
						};
						await update.promise;
						update = Promise.withResolvers();
					}
				} finally {
					state.listeners.delete(changed);
				}
			} catch (e_1) {
				env_1.error = e_1;
				env_1.hasError = true;
			} finally {
				__disposeResources(env_1);
			}
		}
	};
})();
//#endregion
export { SessionToolResults as default };
