//#region lib/types/parse.js
/**
* Batch parser following OpenAI Codex rust-v0.158.0 parser.rs and streaming_parser.rs.
* Adapted into a pure TypeScript parser with original-input diagnostics and no environment routing.
* Copyright 2025 OpenAI. Apache-2.0 portions and modification notice: ../LICENSE.
*/
const BEGIN = "*** Begin Patch";
const END = "*** End Patch";
const EOF = "*** End of File";
const MOVE = "*** Move to: ";
const ADD = "*** Add File: ";
const DELETE = "*** Delete File: ";
const UPDATE = "*** Update File: ";
/** Rust uses Unicode White_Space (all in the BMP); JavaScript trim also removes BOM. */
const WHITE_SPACE$1 = /\p{White_Space}/u;
/** Syntax failure with a one-based line in the original input, including any wrapper. */
var PatchParseError = class extends Error {
	lineNumber;
	reason;
	constructor(lineNumber, reason) {
		super(`Invalid patch at line ${lineNumber}: ${reason}`);
		this.lineNumber = lineNumber;
		this.reason = reason;
		this.name = "PatchParseError";
	}
};
/** Scan the ends to avoid regex backtracking over long interior whitespace. */
function trim(value) {
	let start = 0;
	while (start < value.length && WHITE_SPACE$1.test(value.charAt(start))) start++;
	return trimEnd(value.slice(start));
}
function trimEnd(value) {
	let end = value.length;
	while (end > 0 && WHITE_SPACE$1.test(value.charAt(end - 1))) end--;
	return value.slice(0, end);
}
function patchLines(patch) {
	const offset = patch.slice(0, patch.length - patch.replace(/^\p{White_Space}+/u, "").length).split("\n").length - 1;
	let lines = trim(patch).split(/\r?\n/u).map((text, index) => ({
		text,
		number: offset + index + 1
	}));
	const first = lines[0];
	const last = lines.at(-1);
	if ([
		"<<EOF",
		"<<'EOF'",
		"<<\"EOF\""
	].includes(first.text) && last.text.endsWith("EOF") && lines.length >= 4) lines = lines.slice(1, -1);
	const start = lines[0];
	const end = lines.at(-1);
	if (trim(start.text) !== BEGIN) throw new PatchParseError(start.number, `Expected '${BEGIN}'`);
	if (trim(end.text) !== END) throw new PatchParseError(end.number, `Expected '${END}'`);
	return lines.map((line, index) => index < lines.length - 1 ? {
		...line,
		text: line.text.replace(/\r$/u, "")
	} : line);
}
function emptyHunk(hunk) {
	return hunk.oldLines.length === 0 && hunk.newLines.length === 0;
}
function ensureUpdate(operation, lineNumber) {
	if (operation?.kind !== "update") return;
	const last = operation.hunks.at(-1);
	if (last === void 0) throw new PatchParseError(operation.lineNumber, `Update for '${operation.path}' requires at least one hunk`);
	if (emptyHunk(last)) throw new PatchParseError(lineNumber, "Update hunk must contain context, added, or removed lines");
}
function newHunk(lineNumber) {
	return {
		lineNumber,
		oldLines: [],
		newLines: [],
		isEndOfFile: false
	};
}
function readUpdate(operation, line) {
	const text = trimEnd(line.text);
	const previous = operation.hunks.at(-1);
	const marker = text === "@@" || text.startsWith("@@ ");
	if (previous?.isEndOfFile) {
		if (text === "") return;
		if (!marker) throw new PatchParseError(line.number, "Expected '@@' after '*** End of File'");
	}
	if (previous === void 0 && operation.moveTo === void 0 && text.startsWith(MOVE)) {
		operation.moveTo = text.slice(13);
		return;
	}
	if (marker) {
		if (previous !== void 0 && emptyHunk(previous)) throw new PatchParseError(line.number, "Update hunk must contain context, added, or removed lines");
		const hunk = newHunk(line.number);
		if (text.startsWith("@@ ")) hunk.context = text.slice(3);
		operation.hunks.push(hunk);
		return;
	}
	if (text === EOF) {
		if (previous !== void 0) {
			if (emptyHunk(previous)) throw new PatchParseError(line.number, "Update hunk must contain context, added, or removed lines");
			previous.isEndOfFile = true;
		}
		return;
	}
	const prefix = line.text[0];
	if (line.text !== "" && prefix !== " " && prefix !== "+" && prefix !== "-") throw new PatchParseError(line.number, "Expected a context (' '), added ('+'), removed ('-') line, or '@@'");
	const hunk = previous ?? newHunk(line.number);
	if (previous === void 0) operation.hunks.push(hunk);
	const content = line.text.slice(1);
	if (prefix === "+") hunk.newLines.push(content);
	else if (prefix === "-") hunk.oldLines.push(content);
	else {
		hunk.oldLines.push(content);
		hunk.newLines.push(content);
	}
}
/**
* Parse a complete Codex-style patch without reading or modifying files.
* @param patch - patch text, optionally wrapped in a literal EOF heredoc.
* @returns ordered, JSON-serializable operations; an empty patch returns an empty array.
* @throws PatchParseError for malformed syntax or an unsupported Environment ID header.
*/
function parsePatch(patch) {
	const lines = patchLines(patch);
	const operations = [];
	let ended = false;
	for (const line of lines.slice(1)) {
		const operation = operations.at(-1);
		const header = operation?.kind === "update" && line !== lines.at(-1) ? trimEnd(line.text) : trim(line.text);
		if (ended && line !== lines.at(-1)) {
			if (trim(line.text) !== "") throw new PatchParseError(line.number, `Unexpected content after '${END}'`);
			continue;
		}
		if (header === END) {
			ensureUpdate(operation, line.number);
			ended = true;
		} else if (header.startsWith(ADD)) {
			ensureUpdate(operation, line.number);
			operations.push({
				kind: "add",
				path: header.slice(14),
				lineNumber: line.number,
				content: ""
			});
		} else if (header.startsWith(DELETE)) {
			ensureUpdate(operation, line.number);
			operations.push({
				kind: "delete",
				path: header.slice(17),
				lineNumber: line.number
			});
		} else if (header.startsWith(UPDATE)) {
			ensureUpdate(operation, line.number);
			operations.push({
				kind: "update",
				path: header.slice(17),
				lineNumber: line.number,
				hunks: []
			});
		} else if (operations.length === 0 && header.startsWith("*** Environment ID:")) throw new PatchParseError(line.number, "Environment ID headers are unsupported; the caller selects the filesystem provider");
		else if (operation?.kind === "add" && line.text.startsWith("+")) operation.content += `${line.text.slice(1)}\n`;
		else if (operation?.kind === "update") readUpdate(operation, line);
		else throw new PatchParseError(line.number, "Expected an Add File, Delete File, or Update File header; added content must start with +");
	}
	return operations;
}
//#endregion
//#region lib/types/apply.js
/**
* Pure text updates following OpenAI Codex rust-v0.158.0 NormalizeToLf behavior.
* Adapted from seek_sequence.rs and file_update.rs with structured match failures.
* Copyright 2025 OpenAI. Apache-2.0 portions and modification notice: ../LICENSE.
*/
const WHITE_SPACE = /\p{White_Space}/u;
function trimWhitespace(value, leading) {
	let start = 0;
	let end = value.length;
	if (leading) while (start < end && WHITE_SPACE.test(value.charAt(start))) start++;
	while (end > start && WHITE_SPACE.test(value.charAt(end - 1))) end--;
	return value.slice(start, end);
}
function normalizePunctuation(value) {
	return trimWhitespace(value, true).replace(/[\u2010-\u2015\u2212]/gu, "-").replace(/[\u2018-\u201B]/gu, "'").replace(/[\u201C-\u201F]/gu, "\"").replace(/[\u00A0\u2002-\u200A\u202F\u205F\u3000]/gu, " ");
}
const MATCH_PASSES = [
	(value) => value,
	(value) => trimWhitespace(value, false),
	(value) => trimWhitespace(value, true),
	normalizePunctuation
];
/** NormalizeToLf checks only the tail candidate for EOF, even before the cursor. */
function seekSequence(lines, pattern, start, eof) {
	if (pattern.length === 0) return start;
	if (pattern.length > lines.length) return void 0;
	const last = lines.length - pattern.length;
	const first = eof ? last : start;
	for (const normalize of MATCH_PASSES) {
		const expected = pattern.map(normalize);
		for (let index = first; index <= last; index++) if (expected.every((line, offset) => normalize(lines[index + offset]) === line)) return index;
	}
}
/**
* Apply parsed hunks to LF-normalized text without I/O or input mutation.
* All searches use the original text; a failed search returns no partial text.
* Successful nonempty output ends in LF, including initially unterminated text.
* @param before - text normalized by the caller; CR characters receive no line-ending handling.
* @param hunks - ordered hunks from parsePatch; EOF searches follow NormalizeToLf tail semantics.
* @returns new text or the first unmatched hunk, without throwing for a match failure.
*/
function applyHunks(before, hunks) {
	const original = before.split("\n");
	if (original.at(-1) === "") original.pop();
	const replacements = [];
	let lineIndex = 0;
	for (const hunk of hunks) {
		if (hunk.context !== void 0) {
			const pattern = [hunk.context];
			const found = seekSequence(original, pattern, lineIndex, false);
			if (found === void 0) return {
				ok: false,
				failure: {
					hunkLineNumber: hunk.lineNumber,
					stage: "context",
					searchStart: lineIndex,
					pattern
				}
			};
			lineIndex = found + 1;
		}
		if (hunk.oldLines.length === 0) {
			const start = original.at(-1) === "" ? original.length - 1 : original.length;
			replacements.push({
				start,
				oldLength: 0,
				newLines: hunk.newLines
			});
			continue;
		}
		let pattern = hunk.oldLines;
		let newLines = hunk.newLines;
		let found = seekSequence(original, pattern, lineIndex, hunk.isEndOfFile);
		if (found === void 0 && pattern.at(-1) === "") {
			pattern = pattern.slice(0, -1);
			if (newLines.at(-1) === "") newLines = newLines.slice(0, -1);
			found = seekSequence(original, pattern, lineIndex, hunk.isEndOfFile);
		}
		if (found === void 0) return {
			ok: false,
			failure: {
				hunkLineNumber: hunk.lineNumber,
				stage: "oldLines",
				searchStart: lineIndex,
				pattern: [...hunk.oldLines]
			}
		};
		replacements.push({
			start: found,
			oldLength: pattern.length,
			newLines
		});
		lineIndex = found + pattern.length;
	}
	replacements.sort((left, right) => left.start - right.start);
	let lines = original;
	for (const replacement of replacements.reverse()) lines = [
		...lines.slice(0, replacement.start),
		...replacement.newLines,
		...lines.slice(replacement.start + replacement.oldLength)
	];
	if (lines.at(-1) !== "") lines.push("");
	return {
		ok: true,
		after: lines.join("\n")
	};
}
//#endregion
export { PatchParseError, applyHunks, parsePatch };
