import { useState, useEffect } from 'react';
import { Button } from '../../components/atoms';
import { useAppStore } from '../../stores/appStore';
import {
  startMcpServer,
  stopMcpServer,
  getMcpStatus,
  getMcpBinaryPath,
  getWavsUrl,
  saveMcpSettings,
  registerClaudeMcp,
  pickFolder,
} from '../../tauri';
import { errorMessage } from '../../utils/error';
import type { McpStatus } from '../../types';

export function McpServerSection() {
  const settings = useAppStore((state) => state.settings);

  const [mcpStatus, setMcpStatus] = useState<McpStatus | null>(null);
  const [mcpBinaryPath, setMcpBinaryPath] = useState<string | null>(null);
  const [wavsUrl, setWavsUrl] = useState('http://localhost:8000');
  const [mcpAutoStart, setMcpAutoStart] = useState(settings.mcp_auto_start ?? false);
  const [mcpToken, setMcpToken] = useState(settings.mcp_token ?? '');
  const [mcpLoading, setMcpLoading] = useState(false);
  const [mcpError, setMcpError] = useState<string | null>(null);
  const [claudeProjectPath, setClaudeProjectPath] = useState('');
  const [claudeRegisterResult, setClaudeRegisterResult] = useState<string | null>(null);
  const [claudeRegisterLoading, setClaudeRegisterLoading] = useState(false);
  const [claudeRegisterError, setClaudeRegisterError] = useState<string | null>(null);

  // Poll MCP status every 3 seconds; also resolve the binary path once
  useEffect(() => {
    getMcpBinaryPath().then(setMcpBinaryPath).catch(() => {});
    getWavsUrl().then(setWavsUrl).catch(() => {});

    let cancelled = false;
    const poll = async () => {
      try {
        const status = await getMcpStatus();
        if (!cancelled) setMcpStatus(status);
      } catch {
        // not fatal
      }
    };
    poll();
    const id = setInterval(poll, 3000);
    return () => { cancelled = true; clearInterval(id); };
  }, []);

  const handleMcpToggle = async () => {
    setMcpLoading(true);
    setMcpError(null);
    try {
      if (mcpStatus?.running) {
        await stopMcpServer();
      } else {
        await startMcpServer();
      }
      setMcpStatus(await getMcpStatus());
    } catch (e) {
      setMcpError(errorMessage(e));
    } finally {
      setMcpLoading(false);
    }
  };

  const handleMcpSaveSettings = async () => {
    setMcpError(null);
    try {
      await saveMcpSettings(mcpAutoStart, mcpToken.trim() || null);
    } catch (e) {
      setMcpError(errorMessage(e));
    }
  };

  const handleRegisterClaude = async () => {
    setClaudeRegisterLoading(true);
    setClaudeRegisterError(null);
    setClaudeRegisterResult(null);
    try {
      const result = await registerClaudeMcp(claudeProjectPath.trim());
      setClaudeRegisterResult(result);
    } catch (e) {
      setClaudeRegisterError(errorMessage(e));
    } finally {
      setClaudeRegisterLoading(false);
    }
  };

  return (
    <div id="mcp-server" className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-beige-light text-lg font-semibold">MCP Server</h2>
          {mcpStatus && (
            <span className={`text-xs font-mono px-2 py-0.5 rounded ${
              mcpStatus.running
                ? 'bg-charcoal-dark text-green-4'
                : 'bg-charcoal-dark text-tan-muted'
            }`}>
              {mcpStatus.running ? `Running (pid ${mcpStatus.pid})` : 'Stopped'}
            </span>
          )}
        </div>
        <Button
          text={mcpLoading ? '...' : mcpStatus?.running ? 'Stop' : 'Start'}
          color={mcpStatus?.running ? 'red' : undefined}
          variant="outline"
          onClick={handleMcpToggle}
          disabled={mcpLoading}
        />
      </div>

      <p className="text-tan-muted text-xs">
        Exposes WAVS operations to AI assistants (Claude Desktop, Cursor, VS Code) via the Model Context Protocol.
      </p>

      {/* Auto-start toggle */}
      <label className="flex items-center gap-3 cursor-pointer">
        <input
          type="checkbox"
          checked={mcpAutoStart}
          onChange={(e) => setMcpAutoStart(e.target.checked)}
          className="w-4 h-4 accent-green-4"
        />
        <span className="text-beige-warm text-sm">Auto-start when WAVS node starts</span>
      </label>

      {/* Bearer token */}
      <div className="flex flex-col gap-1">
        <label className="text-tan-muted text-xs">Bearer token (for write operations)</label>
        <div className="flex gap-2">
          <input
            type="password"
            placeholder="Optional -- leave blank for read-only access"
            value={mcpToken}
            onChange={(e) => setMcpToken(e.target.value)}
            className="flex-1 px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
          />
          <Button
            text="Generate"
            variant="outline"
            onClick={() => {
              const bytes = new Uint8Array(24);
              crypto.getRandomValues(bytes);
              setMcpToken(btoa(String.fromCharCode(...bytes)).replace(/[+/=]/g, (c) => ({ '+': '-', '/': '_', '=': '' }[c] ?? c)));
            }}
          />
        </div>
      </div>

      <Button
        text="Save MCP Settings"
        variant="outline"
        onClick={handleMcpSaveSettings}
      />

      {/* Config snippet */}
      <div className="flex flex-col gap-1">
        <span className="text-tan-muted text-xs">Claude Desktop / Cursor config snippet:</span>
        <pre className="text-xs font-mono text-beige-warm bg-charcoal-darkest rounded p-3 overflow-x-auto whitespace-pre-wrap">{
`{
  "mcpServers": {
    "wavs": {
      "command": "${mcpBinaryPath ?? '/path/to/wavs-mcp'}",
      "args": ["--wavs-url", "${wavsUrl}"${mcpToken.trim() ? `,\n               "--token", "${mcpToken.trim()}"` : ''}]
    }
  }
}`
        }</pre>
        {!mcpBinaryPath && (
          <p className="text-tan-muted text-xs mt-1">
            Binary not found. Build it with: <span className="font-mono">cargo build --release -p wavs-mcp</span>
          </p>
        )}
      </div>

      {/* Register with Claude Code */}
      <div className="flex flex-col gap-2">
        <label className="text-tan-muted text-xs font-medium">Register with Claude Code</label>
        <p className="text-tan-muted text-xs">
          Add wavs-mcp to a Claude Code project so MCP tools are available there.
        </p>
        <div className="flex gap-2">
          <input
            type="text"
            value={claudeProjectPath}
            onChange={(e) => setClaudeProjectPath(e.target.value)}
            placeholder="/path/to/your-project"
            className="flex-1 px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
          />
          <Button
            text="Browse..."
            variant="outline"
            onClick={async () => {
              const path = await pickFolder();
              if (path) setClaudeProjectPath(path);
            }}
          />
          <Button
            text={claudeRegisterLoading ? '...' : 'Register'}
            variant="outline"
            onClick={handleRegisterClaude}
            disabled={claudeRegisterLoading || !mcpStatus?.running || !claudeProjectPath.trim()}
          />
        </div>
        {claudeRegisterResult && (
          <p className="text-green-4 text-xs">
            Registered for {claudeRegisterResult}. Restart Claude Code to pick up the change.
          </p>
        )}
        {claudeRegisterError && (
          <p className="text-red-4 text-xs">{claudeRegisterError}</p>
        )}
      </div>

      {mcpError && <p className="text-red-4 text-sm">{mcpError}</p>}
    </div>
  );
}
