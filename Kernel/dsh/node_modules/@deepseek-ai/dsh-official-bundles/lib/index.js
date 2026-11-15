//#region lib/types/bundles.js
/** The bundle package names the dsh installation selects, offers, installs on demand, or retires. */
/** The shipped profile templates auto-initialized on first use, by name. */
const PROFILE_TEMPLATES = {
	acp: { bundles: ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-acp-app"] },
	web: { bundles: ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"] },
	headless: { bundles: ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-headless"] },
	sdk: { bundles: ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-sdk-app"] },
	"sdk-minimal": { bundles: ["@deepseek-ai/dsh-sdk-minimal"] }
};
/**
* Installation-owned bundle tuples that an earlier release wrote into a shipped profile; app-boot replaces an exact
* match with the {@link PROFILE_TEMPLATES} entry of the same name and leaves every other list untouched.
*/
const INSTALLATION_OWNED_PROFILE_TUPLES = { headless: [
	"@deepseek-ai/dsh-base",
	"@deepseek-ai/dsh-web-app",
	"@deepseek-ai/dsh-headless"
] };
/** Bundles an earlier release shipped and the installation no longer carries; app-boot removes them from profiles. */
const RETIRED_BUNDLES = new Set(["@deepseek-ai/dsh-experimental-schedule-bundle", "@deepseek-ai/dsh-experimental-agent-team-profile"]);
/** The bundle list a `dsh plugin` init uses for a name with no shipped template. */
const DEFAULT_PROFILE_BUNDLES = ["@deepseek-ai/dsh-base"];
/**
* The bundles the dsh installation ships for a person to switch on: each a
* runtime dependency of the installation that declares `dsh.bundle.patch`,
* an `icon`, and `./locale/*.json` display metadata, selected by no shipped
* template, and offered switched off by the plugin manager except persistent
* terminals, which have no page switch
* ([rationale](../../../../.agents/notes/implemented/process/2026-09-15-shipped-optional-bundles.md),
* [admission](../../../../.agents/notes/implemented/architecture/2026-09-21-experimental-capabilities-as-optional-bundles.md)).
*/
const OPTIONAL_BUNDLES = [
	"@deepseek-ai/dsh-personalized-recommendations",
	"@deepseek-ai/dsh-cua-bundle",
	"@deepseek-ai/dsh-experimental-session-search",
	"@deepseek-ai/dsh-experimental-ralph-bundle",
	"@deepseek-ai/dsh-experimental-terminal-bundle",
	"@deepseek-ai/dsh-experimental-badge-skill-bundle",
	"@deepseek-ai/dsh-experimental-session-titles-bundle",
	"@deepseek-ai/dsh-experimental-voice-input-bundle",
	"@deepseek-ai/dsh-experimental-cot-translation-bundle",
	"@deepseek-ai/dsh-experimental-auto-review",
	"@deepseek-ai/dsh-experimental-inspector-profile",
	"@deepseek-ai/dsh-experimental-liveshare",
	"@deepseek-ai/dsh-experimental-plugin-market-bundle",
	"@deepseek-ai/dsh-experimental-tool-worktree",
	"@deepseek-ai/dsh-experimental-forge-bundle",
	"@deepseek-ai/dsh-deepseek-chat-import"
];
/** Public DSH release members available through the ordinary on-demand bundle installer. */
const ON_DEMAND_BUNDLES = ["@deepseek-ai/dsh-subagent-claude-code", "@deepseek-ai/dsh-subagent-codex"];
//#endregion
//#region lib/types/plugin-manager.js
/** The package names the plugin manager protects, hides, orders, or marks experimental. */
/**
* Plugin packages the plugin manager refuses to disable or remove, because management itself needs them; a bundle that
* contains one is read-only as well. Removing a name drops only this `management-required` refusal; the manager's own
* entry, installation-carried bundles, and unaddressable rows stay protected by their own rules.
*/
const PROTECTED_PLUGINS = new Set([
	"@deepseek-ai/dsh-plugin-manager",
	"@deepseek-ai/cordis-plugin-loader",
	"@deepseek-ai/cordis-plugin-include",
	"@deepseek-ai/dsh-api-gateway",
	"@deepseek-ai/dsh-host-webserver",
	"@deepseek-ai/dsh-client-modules",
	"@deepseek-ai/dsh-client-ui-settings-plugin-inventory",
	"@deepseek-ai/dsh-client-ui-plugin-manager",
	"@deepseek-ai/dsh-host-plugin-inventory",
	"@deepseek-ai/dsh-typert-registry",
	"@deepseek-ai/dsh-api-remotes",
	"@deepseek-ai/dsh-api-remote-access",
	"@deepseek-ai/cordis-plugin-timer",
	"@deepseek-ai/dsh-client-connection",
	"@deepseek-ai/dsh-host-frontend-static",
	"@deepseek-ai/dsh-tools",
	"@deepseek-ai/dsh-hmr"
]);
/**
* Built-in profile bundles, persistent terminals, and the standalone worktree tool, which have no plugin manager page.
* Their Host availability and activation stay intact.
*/
const HIDDEN_BUNDLES = new Set([
	"@deepseek-ai/dsh-base",
	"@deepseek-ai/dsh-web-app",
	"@deepseek-ai/dsh-headless",
	"@deepseek-ai/dsh-sdk-app",
	"@deepseek-ai/dsh-acp-app",
	"@deepseek-ai/dsh-sdk-minimal",
	"@deepseek-ai/dsh-experimental-terminal-bundle",
	"@deepseek-ai/dsh-experimental-tool-worktree"
]);
/**
* The optional bundles the plugin manager's main and More lists rank first, in this order. {@link MORE_BUNDLES}, not
* this list, decides whether a bundle appears on the main list.
*/
const EXTENSION_BUNDLES = [
	"@deepseek-ai/dsh-experimental-voice-input-bundle",
	"@deepseek-ai/dsh-experimental-plugin-market-bundle",
	"@deepseek-ai/dsh-experimental-auto-review"
];
/**
* The optional bundles the plugin manager's main list leaves out until the profile enables them; its More list,
* which includes every eligible bundle, ranks them last in this order.
*/
const MORE_BUNDLES = [
	"@deepseek-ai/dsh-personalized-recommendations",
	"@deepseek-ai/dsh-experimental-inspector-profile",
	"@deepseek-ai/dsh-cua-bundle",
	"@deepseek-ai/dsh-experimental-cot-translation-bundle",
	"@deepseek-ai/dsh-experimental-session-search",
	"@deepseek-ai/dsh-experimental-session-titles-bundle",
	"@deepseek-ai/dsh-experimental-badge-skill-bundle",
	"@deepseek-ai/dsh-experimental-ralph-bundle",
	"@deepseek-ai/dsh-deepseek-chat-import",
	"@deepseek-ai/dsh-subagent-codex",
	"@deepseek-ai/dsh-subagent-claude-code"
];
/** Official bundles the plugin manager marks with the Experimental badge. */
const EXPERIMENTAL_BUNDLES = new Set([
	"@deepseek-ai/dsh-personalized-recommendations",
	"@deepseek-ai/dsh-cua-bundle",
	"@deepseek-ai/dsh-experimental-voice-input-bundle",
	"@deepseek-ai/dsh-experimental-auto-review",
	"@deepseek-ai/dsh-experimental-inspector-profile",
	"@deepseek-ai/dsh-experimental-forge-bundle",
	"@deepseek-ai/dsh-experimental-tool-worktree",
	"@deepseek-ai/dsh-experimental-cot-translation-bundle",
	"@deepseek-ai/dsh-experimental-session-search",
	"@deepseek-ai/dsh-experimental-session-titles-bundle",
	"@deepseek-ai/dsh-experimental-badge-skill-bundle",
	"@deepseek-ai/dsh-experimental-ralph-bundle",
	"@deepseek-ai/dsh-experimental-plugin-market-bundle",
	"@deepseek-ai/dsh-experimental-liveshare",
	"@deepseek-ai/dsh-deepseek-chat-import"
]);
//#endregion
//#region lib/types/catalog.generated.js
/** Generated offline Official bundle metadata. Run pnpm gen-official-bundle-catalog; do not edit. */
/** Official on-demand entries, independent of installed packages and registry availability. */
const OFFICIAL_ON_DEMAND_CATALOG = [{
	packageName: "@deepseek-ai/dsh-subagent-claude-code",
	meta: {
		title: {
			"de": "Claude Code-Subagent",
			"en": "Claude Code subagent",
			"es": "Subagente Claude Code",
			"fr": "Sous-agent Claude Code",
			"ja": "Claude Code サブエージェント",
			"pt": "Claude Code subagente",
			"ru": "Субагент Claude Code",
			"zh": "Claude Code 子智能体"
		},
		description: {
			"de": "Delegieren Sie eine Aufgabe an Claude Code, indem Sie dessen native Authentifizierung und Einstellungen verwenden.",
			"en": "Delegate one task to Claude Code using its native authentication and settings.",
			"es": "Delegue una tarea a Claude Code utilizando su autenticación y configuración nativas.",
			"fr": "Déléguez une tâche à Claude Code à l’aide de son authentification et de ses paramètres natifs.",
			"ja": "Claude Code のネイティブの認証と設定を使用して、単独のタスクを委派します。",
			"pt": "Delegue uma tarefa a Claude Code usando sua autenticação e configurações nativas.",
			"ru": "Делегируйте одну задачу Claude Code, используя его встроенную аутентификацию и настройки.",
			"zh": "使用 Claude Code 原生认证和设置委派单个任务。"
		},
		icon: "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMzYiIGhlaWdodD0iMzYiIHZpZXdCb3g9IjAgMCAzNiAzNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB4PSIxNC45ODkzIiB5PSIxNS4xODc3IiB3aWR0aD0iMTIuMjM2NSIgaGVpZ2h0PSIxMi4yMzY1IiByeD0iMiIgZmlsbD0idXJsKCNkc2hfcGx1Z2luX2FydF9SMGEpIiBmaWxsLW9wYWNpdHk9IjAuOCI+PC9yZWN0PjxyZWN0IHg9IjguODcxMDkiIHk9IjkuMDY5NyIgd2lkdGg9IjEyLjIzNjUiIGhlaWdodD0iMTIuMjM2NSIgcng9IjIiIGZpbGw9InVybCgjZHNoX3BsdWdpbl9hcnRfUjBiKSIgZmlsbC1vcGFjaXR5PSIwLjgiPjwvcmVjdD48ZGVmcz48bGluZWFyR3JhZGllbnQgaWQ9ImRzaF9wbHVnaW5fYXJ0X1IwYSIgeDE9IjIxLjEwNzUiIHkxPSIxNS4xODc3IiB4Mj0iMjEuMTA3NSIgeTI9IjI3LjQyNDMiIGdyYWRpZW50VW5pdHM9InVzZXJTcGFjZU9uVXNlIj48c3RvcCBzdG9wLWNvbG9yPSIjNDVFN0E0Ij48L3N0b3A+PHN0b3Agb2Zmc2V0PSIxIiBzdG9wLWNvbG9yPSIjMDU5MDlEIj48L3N0b3A+PC9saW5lYXJHcmFkaWVudD48bGluZWFyR3JhZGllbnQgaWQ9ImRzaF9wbHVnaW5fYXJ0X1IwYiIgeDE9IjE0Ljk4OTQiIHkxPSI5LjA2OTciIHgyPSIxNC45ODk0IiB5Mj0iMjEuMzA2MyIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiPjxzdG9wIHN0b3AtY29sb3I9IiM2OUI5RkYiPjwvc3RvcD48c3RvcCBvZmZzZXQ9IjEiIHN0b3AtY29sb3I9IiMzMjRERTIiPjwvc3RvcD48L2xpbmVhckdyYWRpZW50PjwvZGVmcz48L3N2Zz4K"
	}
}, {
	packageName: "@deepseek-ai/dsh-subagent-codex",
	meta: {
		title: {
			"de": "Codex-Subagent",
			"en": "Codex subagent",
			"es": "Subagente Codex",
			"fr": "Sous-agent Codex",
			"ja": "Codex サブエージェント",
			"pt": "Codex subagente",
			"ru": "Субагент Codex",
			"zh": "Codex 子智能体"
		},
		description: {
			"de": "Delegieren Sie eine Aufgabe an Codex, indem Sie dessen native Authentifizierung und Einstellungen verwenden.",
			"en": "Delegate one task to Codex using its native authentication and settings.",
			"es": "Delegue una tarea a Codex utilizando su autenticación y configuración nativas.",
			"fr": "Déléguez une tâche à Codex à l’aide de son authentification et de ses paramètres natifs.",
			"ja": "Codex のネイティブの認証と設定を使用して、単独のタスクを委派します。",
			"pt": "Delegue uma tarefa a Codex usando sua autenticação e configurações nativas.",
			"ru": "Делегируйте одну задачу Codex, используя его встроенную аутентификацию и настройки.",
			"zh": "使用 Codex 原生认证和设置委派单个任务。"
		},
		icon: "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMzYiIGhlaWdodD0iMzYiIHZpZXdCb3g9IjAgMCAzNiAzNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB4PSIxNC45ODkzIiB5PSIxNS4xODc3IiB3aWR0aD0iMTIuMjM2NSIgaGVpZ2h0PSIxMi4yMzY1IiByeD0iMiIgZmlsbD0idXJsKCNkc2hfcGx1Z2luX2FydF9SMGEpIiBmaWxsLW9wYWNpdHk9IjAuOCI+PC9yZWN0PjxyZWN0IHg9IjguODcxMDkiIHk9IjkuMDY5NyIgd2lkdGg9IjEyLjIzNjUiIGhlaWdodD0iMTIuMjM2NSIgcng9IjIiIGZpbGw9InVybCgjZHNoX3BsdWdpbl9hcnRfUjBiKSIgZmlsbC1vcGFjaXR5PSIwLjgiPjwvcmVjdD48ZGVmcz48bGluZWFyR3JhZGllbnQgaWQ9ImRzaF9wbHVnaW5fYXJ0X1IwYSIgeDE9IjIxLjEwNzUiIHkxPSIxNS4xODc3IiB4Mj0iMjEuMTA3NSIgeTI9IjI3LjQyNDMiIGdyYWRpZW50VW5pdHM9InVzZXJTcGFjZU9uVXNlIj48c3RvcCBzdG9wLWNvbG9yPSIjNDVFN0E0Ij48L3N0b3A+PHN0b3Agb2Zmc2V0PSIxIiBzdG9wLWNvbG9yPSIjMDU5MDlEIj48L3N0b3A+PC9saW5lYXJHcmFkaWVudD48bGluZWFyR3JhZGllbnQgaWQ9ImRzaF9wbHVnaW5fYXJ0X1IwYiIgeDE9IjE0Ljk4OTQiIHkxPSI5LjA2OTciIHgyPSIxNC45ODk0IiB5Mj0iMjEuMzA2MyIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiPjxzdG9wIHN0b3AtY29sb3I9IiM2OUI5RkYiPjwvc3RvcD48c3RvcCBvZmZzZXQ9IjEiIHN0b3AtY29sb3I9IiMzMjRERTIiPjwvc3RvcD48L2xpbmVhckdyYWRpZW50PjwvZGVmcz48L3N2Zz4K"
	}
}];
//#endregion
export { DEFAULT_PROFILE_BUNDLES, EXPERIMENTAL_BUNDLES, EXTENSION_BUNDLES, HIDDEN_BUNDLES, INSTALLATION_OWNED_PROFILE_TUPLES, MORE_BUNDLES, OFFICIAL_ON_DEMAND_CATALOG, ON_DEMAND_BUNDLES, OPTIONAL_BUNDLES, PROFILE_TEMPLATES, PROTECTED_PLUGINS, RETIRED_BUNDLES };
