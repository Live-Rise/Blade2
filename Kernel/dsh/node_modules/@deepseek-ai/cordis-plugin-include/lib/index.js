import { EntryGroup, EntryTree, isJsExpr } from "@deepseek-ai/cordis-plugin-loader";
import { Service } from "@deepseek-ai/cordis";
import { deepEqual, omit } from "@deepseek-ai/cosmokit";
import { basename, dirname, extname, join } from "node:path";
import { access, constants, readFile, rename, rm, writeFile } from "node:fs/promises";
import { setTimeout } from "node:timers/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import * as yaml from "js-yaml";
//#region lib/types/journal.js
function diffOptions(legacy, options) {
	const changes = {};
	for (const key of new Set([...Object.keys(legacy), ...Object.keys(options)])) {
		if (key === "id") continue;
		if (deepEqual(legacy[key], options[key])) continue;
		changes[key] = options[key];
	}
	return changes;
}
/** Fold a later record into an earlier one for the same entry. */
function mergeRecords(older, newer) {
	if (!older) return newer;
	if (newer.kind === "remove") return older.kind === "upsert" && older.created ? void 0 : newer;
	if (older.kind === "remove") return {
		...newer,
		created: false
	};
	const relocated = newer.parent !== void 0;
	return {
		kind: "upsert",
		created: older.created,
		parent: relocated ? newer.parent : older.parent,
		position: relocated ? newer.position : older.position,
		changes: {
			...older.changes,
			...newer.changes
		}
	};
}
function merge(journal, id, record) {
	const merged = mergeRecords(journal.get(id), record);
	if (merged) journal.set(id, merged);
	else journal.delete(id);
}
function record(journal, change, parent) {
	const { id, options, legacy } = change;
	if (!options) return merge(journal, id, { kind: "remove" });
	const position = change.group.data.indexOf(options);
	if (!legacy) return merge(journal, id, {
		kind: "upsert",
		created: true,
		parent,
		position,
		changes: omit(options, ["id"])
	});
	merge(journal, id, {
		kind: "upsert",
		created: false,
		parent: change.from ? parent : void 0,
		position,
		changes: diffOptions(legacy, options)
	});
}
/** Index every entry (including nested group members) by id. */
function flatten(data, parent = null, result = /* @__PURE__ */ new Map()) {
	data.forEach((options, position) => {
		if (options.id) result.set(options.id, {
			parent,
			position,
			options,
			list: data
		});
		if (options.group && Array.isArray(options.config)) flatten(options.config, options.id ?? null, result);
	});
	return result;
}
function applyChanges(options, changes, filter) {
	for (const [key, value] of Object.entries(changes)) {
		if (filter && !filter(key)) continue;
		if (value === void 0) delete options[key];
		else options[key] = value;
	}
}
function detach(data, id) {
	const flat = flatten(data).get(id);
	if (!flat) return;
	flat.list.splice(flat.list.indexOf(flat.options), 1);
	return flat.options;
}
/**
* Insert `options` under `parent` (a group id, or `null` for the root) at
* `position`, clamped to the group's length. Returns false when the parent
* group does not exist.
*/
function place(data, options, parent, position) {
	let list = data;
	if (parent !== null) {
		const group = flatten(data).get(parent);
		if (!group?.options.group) return false;
		list = group.options.config ??= [];
	}
	list.splice(Math.min(position, list.length), 0, options);
	return true;
}
/**
* Overlay a journal onto `data` in place. `filter(id, key)` restricts which
* changes apply; `key` is `null` when asking about the entry as a whole
* (creation, removal, relocation).
*/
function applyJournal(data, journal, filter = () => true, warn = () => {}) {
	for (const [id, record] of journal) {
		if (!filter(id, null)) continue;
		if (record.kind === "remove") {
			detach(data, id);
			continue;
		}
		const flat = flatten(data).get(id);
		if (!flat) {
			const options = { id };
			applyChanges(options, record.changes, (key) => filter(id, key));
			const parent = record.parent ?? null;
			if (!place(data, options, parent, record.position)) warn("cannot place entry %C: group %C not found", id, parent);
			continue;
		}
		applyChanges(flat.options, record.changes, (key) => filter(id, key));
		if (record.parent !== void 0 && flat.parent !== record.parent) {
			detach(data, id);
			if (!place(data, flat.options, record.parent, record.position)) {
				warn("cannot move entry %C: group %C not found", id, record.parent);
				place(data, flat.options, flat.parent, flat.position);
			}
		}
	}
	return data;
}
/**
* Three-way reconciliation of a journal against a new file state. The file
* wins every conflict: conflicting keys are dropped from the journal so that
* the tree follows the file, and each drop is reported.
*
* `fileOwned(id, key)` scopes the check to what the file can actually express
* (patch-owned keys and patch-inserted entries live elsewhere).
*/
function reconcile(journal, base, theirs, fileOwned) {
	const conflicts = [];
	for (const [id, record] of journal) {
		if (!fileOwned(id, null)) continue;
		const b = base.get(id);
		const t = theirs.get(id);
		if (record.kind === "remove") {
			if (!t) journal.delete(id);
			else if (!b || !deepEqual(t.options, b.options)) {
				conflicts.push({
					id,
					reason: b ? "modified in file, removed at runtime" : "added in file, removed at runtime"
				});
				journal.delete(id);
			}
			continue;
		}
		if (!t) {
			if (b) {
				conflicts.push({
					id,
					reason: "removed in file, modified at runtime"
				});
				journal.delete(id);
			}
			continue;
		}
		if (record.created) record.created = false;
		for (const key of Object.keys(record.changes)) {
			if (!fileOwned(id, key)) continue;
			const bv = b?.options[key];
			const tv = t.options[key];
			if (deepEqual(bv, tv) || deepEqual(tv, record.changes[key])) continue;
			conflicts.push({
				id,
				reason: `key "${key}" modified both in file and at runtime`
			});
			delete record.changes[key];
		}
		if (record.parent !== void 0 && b && t.parent !== b.parent && t.parent !== record.parent) {
			conflicts.push({
				id,
				reason: "moved both in file and at runtime"
			});
			record.parent = void 0;
		}
		const moved = record.parent !== void 0 && record.parent !== t.parent;
		if (!Object.keys(record.changes).length && !moved) journal.delete(id);
	}
	return conflicts;
}
//#endregion
//#region lib/types/patch.js
function randomId(used) {
	let id;
	do
		id = Math.random().toString(16).slice(2, 10);
	while (used(id));
	return id;
}
/** Give every entry (nested groups included) an id, in place. */
function ensureIds(entries, used) {
	for (const entry of entries) {
		if (!entry.id) entry.id = randomId(used);
		if (entry.group && Array.isArray(entry.config)) ensureIds(entry.config, used);
	}
}
/** Runtime changes are routed back to an inserted entry by its id, so every insert needs one. */
function ensureInsertIds(patches, used) {
	for (const patch of patches ?? []) if (patch.insert) ensureIds(patch.insert, used);
}
/**
* Overlay `patches` onto a fresh clone of `data`. Inserted entries are cloned
* too, so the tree owns every object it mounts.
*/
function applyPatches(data, patches, warn) {
	data = structuredClone(data);
	if (!patches?.length) return data;
	const entryMap = /* @__PURE__ */ new Map();
	const buildMap = (entries) => {
		for (const entry of entries) {
			if (entry.id) entryMap.set(entry.id, entry);
			if (entry.group && Array.isArray(entry.config)) buildMap(entry.config);
		}
	};
	buildMap(data);
	for (const patch of patches) {
		const { id, insert, name, ...overrides } = patch;
		if (insert) {
			const clone = structuredClone(insert);
			if (id) {
				const target = entryMap.get(id);
				if (!target) {
					warn("patch insert: entry %C not found", id);
					continue;
				}
				if (!target.group) {
					warn("patch insert: entry %C is not a group", id);
					continue;
				}
				if (!Array.isArray(target.config)) target.config = [];
				target.config.push(...clone);
			} else data.push(...clone);
			buildMap(clone);
			continue;
		}
		if (!id) {
			warn("patch: id is required for non-insert patches");
			continue;
		}
		const target = entryMap.get(id);
		if (!target) {
			warn("patch: entry %C not found", id);
			continue;
		}
		if (name && name !== target.name) {
			warn("patch: name mismatch for %C (expected %C, got %C), skipping", id, target.name, name);
			continue;
		}
		for (const [key, value] of Object.entries(overrides)) {
			if (key === "id") continue;
			target[key] = value;
		}
	}
	return data;
}
var PatchIndex = class {
	patches;
	keys = /* @__PURE__ */ new Map();
	inserts = /* @__PURE__ */ new Map();
	constructor(patches) {
		this.patches = patches;
		patches.forEach((patch, index) => {
			const { id, insert } = patch;
			if (insert) {
				const walk = (list, parent) => {
					for (const options of list) {
						this.inserts.set(options.id, {
							type: "insert",
							index,
							list,
							options,
							parent
						});
						if (options.group && Array.isArray(options.config)) walk(options.config, options.id);
					}
				};
				walk(insert, id ?? null);
				return;
			}
			if (!id) return;
			const keys = this.keys.get(id) ?? /* @__PURE__ */ new Map();
			for (const key of Object.keys(omit(patch, [
				"id",
				"insert",
				"name"
			]))) keys.set(key, index);
			this.keys.set(id, keys);
		});
	}
	/** Owner of the entry as a whole. */
	entry(id) {
		if (id === null) return { type: "file" };
		return this.inserts.get(id) ?? { type: "file" };
	}
	/** Owner of one key of an entry. */
	key(id, key) {
		const insert = this.inserts.get(id);
		if (insert) return insert;
		const index = this.keys.get(id)?.get(key);
		return index === void 0 ? { type: "file" } : {
			type: "patch",
			index
		};
	}
	fileOwned(id, key) {
		return (key === null ? this.entry(id) : this.key(id, key)).type === "file";
	}
};
/**
* Route a journal to its owners: file-owned changes are applied to `data`,
* patch-owned ones to `patches`. Both are mutated in place, so pass clones.
* Returns whether any patch changed.
*/
function routeJournal(journal, data, patches, warn) {
	const index = new PatchIndex(patches);
	let patched = false;
	const insertList = (owner) => owner.options.config ??= [];
	const placeAt = (options, parent, position) => {
		const owner = index.entry(parent);
		if (owner.type === "insert") {
			const list = insertList(owner);
			list.splice(Math.min(position, list.length), 0, options);
			patched = true;
		} else if (!place(data, options, parent, position)) warn("cannot place entry %C: group %C not found", options.id, parent);
	};
	for (const [id, record] of journal) {
		const owner = index.entry(id);
		if (record.kind === "remove") {
			if (owner.type === "insert") {
				owner.list.splice(owner.list.indexOf(owner.options), 1);
				patched = true;
			} else detach(data, id);
			continue;
		}
		const current = owner.type === "insert" ? owner : flatten(data).get(id);
		if (!current) {
			const options = { id };
			applyChanges(options, record.changes);
			placeAt(options, record.parent ?? null, record.position);
			continue;
		}
		if (record.parent !== void 0 && current.parent !== record.parent) {
			if (owner.type === "insert") {
				owner.list.splice(owner.list.indexOf(owner.options), 1);
				patched = true;
			} else detach(data, id);
			applyChanges(current.options, record.changes);
			placeAt(current.options, record.parent, record.position);
			continue;
		}
		if (owner.type === "insert") {
			applyChanges(current.options, record.changes);
			patched = true;
			continue;
		}
		const patchChanges = {};
		applyChanges(current.options, record.changes, (key) => {
			const keyOwner = index.key(id, key);
			if (keyOwner.type !== "patch") return true;
			(patchChanges[keyOwner.index] ??= {})[key] = record.changes[key];
			return false;
		});
		for (const [i, changes] of Object.entries(patchChanges)) {
			applyChanges(patches[i], changes);
			patched = true;
		}
	}
	return patched;
}
//#endregion
//#region lib/types/index.js
const JsExpr = new yaml.Type("tag:yaml.org,2002:js", {
	kind: "scalar",
	resolve: (data) => typeof data === "string",
	construct: (data) => ({ __jsExpr: data }),
	predicate: isJsExpr,
	represent: (data) => data["__jsExpr"]
});
/** YAML dialect preserving child-owned JavaScript expression nodes. */
const entryListSchema = yaml.JSON_SCHEMA.extend(JsExpr);
const schema = entryListSchema;
const types = {
	".json": "application/json",
	".yaml": "application/yaml",
	".yml": "application/yaml"
};
var ConfigFileError = class extends Error {
	stage;
	name = "ConfigFileError";
	constructor(stage, path, cause) {
		super(`failed to ${stage} config file ${path}`, { cause });
		this.stage = stage;
	}
};
/** The file changed under us between the pre-check and the rename. */
var StaleWriteError = class extends Error {
	name = "StaleWriteError";
};
function isENOENT(error) {
	return error?.code === "ENOENT";
}
const RENAME_RETRIES = 10;
const RENAME_BACKOFF = 20;
/**
* On Windows a rename over a file that another process holds open (typically
* an antivirus scanning what we just wrote) fails with one of these; the lock
* is normally gone within milliseconds. A bounded retry is cheap enough to
* apply on every platform.
*/
function isTransientRenameError(error) {
	const code = error?.code;
	return code === "EACCES" || code === "EPERM" || code === "EBUSY";
}
var Include = class extends EntryTree {
	config;
	static inject = ["loader"];
	static [EntryGroup.key] = true;
	filename;
	type;
	/** The last file state we read or wrote. */
	cache;
	/** Tree-side changes that have not reached the file yet. */
	journal = /* @__PURE__ */ new Map();
	/** Ids we assigned to entries the file leaves anonymous. */
	anonymous = /* @__PURE__ */ new Map();
	dirtyRead = false;
	dirtyWrite = false;
	_draining;
	/** Only the latest intended tree is ever applied; older ones are skipped. */
	_pendingTree;
	_applyTask;
	_disposed = false;
	_seq = 0;
	persistenceError;
	constructor(ctx, config) {
		super(ctx);
		this.config = config;
		this.enableLogs = config.enableLogs ?? ctx.fiber.entry?.parent.tree.enableLogs ?? false;
		this.filename = fileURLToPath(new URL(this.config.path, this.ctx.baseUrl));
		const ext = extname(this.filename);
		if (!types[ext]) throw new Error(`extension "${ext}" not supported`);
		this.type = types[ext];
		this.ctx.baseUrl = new URL(".", pathToFileURL(this.filename)).href;
		ensureInsertIds(config.patches, (id) => !!this.store[id]);
		ctx.on("internal/update", (config, next) => {
			if (config.path !== this.config.path) return next();
			this.config = config;
			ensureInsertIds(config.patches, (id) => !!this.store[id]);
			this._scheduleApply();
			return this._applyTask;
		});
	}
	warn = (format, ...args) => {
		this.ctx.root.logger?.("loader").warn(format, ...args);
	};
	async *[Service.init]() {
		try {
			await this._read();
		} catch (error) {
			if (!isENOENT(error instanceof ConfigFileError && error.stage === "read" ? error.cause : void 0)) throw error;
			if (!this.config.initial) throw new Error(`config file not found: ${this.filename}`);
			const initial = structuredClone(this.config.initial);
			ensureIds(initial, (id) => !!this.store[id]);
			await this._writeText(this.dump(initial));
			await this._read();
		}
		yield () => this.stop();
		await this._applyTask;
	}
	async stop() {
		this._disposed = true;
		this._pendingTree = void 0;
		if (this.journal.size) {
			this.dirtyWrite = true;
			this._drain();
		}
		try {
			await this.flush();
		} finally {
			this.root.stop();
		}
	}
	/**
	* Resolves once every pending read and write has been processed and the
	* tree reflects the outcome. Retries pending writes and rejects while changes remain unsaved.
	*/
	async flush() {
		if (this.journal.size && !this._draining) {
			this.dirtyWrite = true;
			this._drain();
		}
		while (this._draining || this._applyTask) {
			await this._draining;
			await this._applyTask;
		}
		if (this.journal.size) throw this.persistenceError ?? /* @__PURE__ */ new Error(`config file ${this.filename} has ${this.journal.size} unsaved change(s)`);
		this.persistenceError = void 0;
	}
	/** Re-read the file and reconcile the tree with it. */
	refresh() {
		this.dirtyRead = true;
		this._drain();
		return this.flush();
	}
	/** Called synchronously by the loader once a tree-side change is in place. */
	commit(change) {
		this.context.emit("loader/config-update");
		const parent = change.group === this.root ? null : change.group.ctx.fiber.entry.options.id;
		record(this.journal, change, parent);
		this.dirtyWrite = true;
		this._drain();
	}
	_drain() {
		if (this._draining) return;
		this._draining = this._loop().finally(() => {
			this._draining = void 0;
		});
	}
	async _loop() {
		while (this.dirtyRead || this.dirtyWrite) {
			if (this.dirtyRead) {
				this.dirtyRead = false;
				try {
					await this._read();
				} catch (error) {
					this.persistenceError = error;
					this.ctx.logger.warn(error);
					if (this.dirtyRead) continue;
					return;
				}
			}
			if (this.dirtyWrite) {
				this.dirtyWrite = false;
				try {
					await this._write();
				} catch (error) {
					this.persistenceError = error;
					this.ctx.logger.error(error);
				}
			}
		}
	}
	async _read() {
		let content;
		try {
			content = await readFile(this.filename, "utf8");
		} catch (error) {
			throw new ConfigFileError("read", this.filename, error);
		}
		if (content === this.cache?.content) return;
		let data;
		try {
			data = this.type === "application/yaml" ? yaml.load(content, { schema }) : JSON.parse(content);
		} catch (error) {
			throw new ConfigFileError("parse", this.filename, error);
		}
		if (!Array.isArray(data)) throw new ConfigFileError("validate", this.filename, /* @__PURE__ */ new TypeError("config file must be a top-level array"));
		this._assignIds(data);
		if (this.cache) {
			const index = new PatchIndex(this.config.patches ?? []);
			const conflicts = reconcile(this.journal, flatten(this.cache.data), flatten(data), (id, key) => index.fileOwned(id, key));
			for (const { id, reason } of conflicts) this.ctx.logger.error("config conflict in %C: entry %C %s; file wins", this.filename, id, reason);
		}
		this.cache = {
			content,
			data
		};
		this._scheduleApply();
		if (this.journal.size) this.dirtyWrite = true;
	}
	async _write() {
		if (!this.cache || !this.journal.size) return;
		const current = await readFile(this.filename, "utf8").catch((error) => {
			if (isENOENT(error)) return void 0;
			throw error;
		});
		if (current === void 0) {
			this.ctx.logger.warn("config file %C is missing, %d pending change(s) kept in memory", this.filename, this.journal.size);
			return;
		}
		if (current !== this.cache.content) {
			this.dirtyRead = true;
			this.dirtyWrite = true;
			return;
		}
		const batch = this.journal;
		this.journal = /* @__PURE__ */ new Map();
		const restore = () => {
			for (const [id, record] of this.journal) merge(batch, id, record);
			this.journal = batch;
		};
		const data = structuredClone(this.cache.data);
		const patches = structuredClone(this.config.patches ?? []);
		if (routeJournal(batch, data, patches, this.warn)) {
			this.config = {
				...this.config,
				patches
			};
			if (this.ctx.fiber.entry) try {
				Promise.resolve(this.ctx.fiber.update(this.config)).catch((error) => {
					this.ctx.logger.error(error);
				});
			} catch (error) {
				this.ctx.logger.error(error);
			}
			else this.ctx.logger.warn("patches of %C changed at runtime but cannot be persisted", this.filename);
		}
		if (deepEqual(data, this.cache.data)) {
			this._scheduleApply();
			return;
		}
		const text = this.dump(data);
		try {
			await access(this.filename, constants.W_OK);
		} catch {
			restore();
			this.ctx.logger.warn("config file %C is read-only, %d pending change(s) kept in memory", this.filename, this.journal.size);
			return;
		}
		try {
			await this._writeText(text, this.cache.content);
		} catch (error) {
			restore();
			if (error instanceof StaleWriteError) {
				this.dirtyRead = true;
				this.dirtyWrite = true;
				return;
			}
			this.ctx.logger.warn("failed to write config file %C, %d pending change(s) kept in memory", this.filename, this.journal.size);
			throw error;
		}
		this.cache = {
			content: text,
			data
		};
		this._scheduleApply();
	}
	/**
	* Write through a temp file and a rename, so that no reader ever sees a
	* truncated file. When `expected` is given, the rename (and every retry of
	* it) only goes ahead while the file still holds that content.
	*/
	async _writeText(text, expected) {
		const tmp = join(dirname(this.filename), `.${basename(this.filename)}.${process.pid}.${this._seq++}.tmp`);
		try {
			await writeFile(tmp, text);
			for (let attempt = 0;; attempt++) {
				if (expected !== void 0) {
					if (await readFile(this.filename, "utf8").catch((error) => {
						if (isENOENT(error)) return void 0;
						throw error;
					}) !== expected) throw new StaleWriteError();
				}
				try {
					await rename(tmp, this.filename);
					return;
				} catch (error) {
					if (attempt >= RENAME_RETRIES || !isTransientRenameError(error)) throw error;
				}
				await setTimeout(RENAME_BACKOFF * (attempt + 1));
			}
		} catch (error) {
			await rm(tmp, { force: true }).catch(() => {});
			throw error;
		}
	}
	dump(data) {
		return this.type === "application/yaml" ? yaml.dump(data, { schema }) : JSON.stringify(data, null, 2);
	}
	/**
	* Entries the file leaves anonymous get an id here, before the loader sees
	* them. One that looks exactly like an entry we named on the previous read
	* keeps that id, so editing a neighbor does not restart it; anything else
	* gets a fresh id. Assigned ids stay in memory until the next write.
	*/
	_assignIds(data) {
		const used = /* @__PURE__ */ new Set();
		const pending = [];
		const walk = (entries, parent) => {
			for (const options of entries) {
				if (options.id) used.add(options.id);
				else pending.push({
					options,
					parent
				});
				if (options.group && Array.isArray(options.config)) walk(options.config, options.id ?? null);
			}
		};
		walk(data, null);
		const previous = this.anonymous;
		this.anonymous = /* @__PURE__ */ new Map();
		for (const { options, parent } of pending) {
			let id;
			for (const [candidate, info] of previous) {
				if (info.parent !== parent || !deepEqual(info.options, options)) continue;
				id = candidate;
				previous.delete(candidate);
				break;
			}
			if (!id) do
				id = Math.random().toString(16).slice(2, 10);
			while (used.has(id) || this.store[id]);
			used.add(id);
			this.anonymous.set(id, {
				parent,
				options: structuredClone(options)
			});
			const rest = { ...options };
			for (const key of Object.keys(options)) delete options[key];
			Object.assign(options, { id }, rest);
		}
	}
	_scheduleApply() {
		if (this._disposed || !this.cache) return;
		const tree = applyPatches(this.cache.data, this.config.patches, this.warn);
		this._pendingTree = applyJournal(tree, this.journal, void 0, this.warn);
		this._applyTask ??= (async () => {
			try {
				while (this._pendingTree) {
					const tree = this._pendingTree;
					this._pendingTree = void 0;
					await this.root.update(tree);
				}
			} finally {
				this._applyTask = void 0;
			}
		})();
	}
};
//#endregion
export { ConfigFileError, Include, Include as default, applyPatches as applyEntryPatches, entryListSchema };
