/** A refused agent terminal operation; the terminal and its other attachments are unchanged. */
export class AgentTerminalError extends Error {
    code;
    /**
     * @param code - machine-routable refusal reason.
     * @param message - English description for logs and model-facing errors.
     */
    constructor(code, message) {
        super(message);
        this.code = code;
        this.name = 'AgentTerminalError';
    }
}
/** The controller's {@link AgentTerminal} over one committed Session terminal; the package does not export it. */
export class AgentTerminalHandle {
    terminal;
    closeTerminal;
    /**
     * @param terminal - committed Session terminal.
     * @param closeTerminal - closes the identity through its Session owner.
     */
    constructor(terminal, closeTerminal) {
        this.terminal = terminal;
        this.closeTerminal = closeTerminal;
    }
    get id() { return this.terminal.info.id; }
    get pid() { return this.terminal.pid; }
    get info() { return this.terminal.info; }
    get revision() { return this.terminal.revision; }
    changed(after, signal) { return this.terminal.changed(after, signal); }
    write(data) { return this.terminal.agentWrite(data); }
    drain() { return this.terminal.drain(); }
    read(request) { return this.terminal.readScreen(request); }
    mark() { return this.terminal.markScreen(); }
    inspectForeground() { return this.terminal.inspectForeground(); }
    signal(signal) { return this.terminal.agentSignal(signal); }
    async close() {
        if (this.terminal.info.control !== 'agent')
            throw new AgentTerminalError('user-control', 'The user has taken over this terminal');
        await this.closeTerminal();
    }
}
//# sourceMappingURL=agent.js.map