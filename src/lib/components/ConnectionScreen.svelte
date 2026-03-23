<script lang="ts">
	import { onMount } from 'svelte';
	import { setStoredWsUrl, getStoredWsUrl, clearStoredWsUrl, wsClient } from '$lib/ws';
	import { initializeSessionListeners, sessions } from '$lib/stores/sessions';
	import { getSessions } from '$lib/api';

	let { onconnected }: { onconnected: () => void } = $props();

	let connecting = $state(false);
	let urlInput = $state('');
	let inputError = $state('');

	const WS_PORT = 9210;

	async function doConnect(wsUrl: string) {
		connecting = true;
		inputError = '';
		setStoredWsUrl(wsUrl);
		try {
			await initializeSessionListeners();
			if (!wsClient.isConnected) {
				clearStoredWsUrl();
				throw new Error('Could not connect — check that the desktop app is running and you are on the same Tailscale network');
			}
			const initial = await getSessions();
			sessions.set(initial);
			onconnected();
		} catch (e) {
			inputError = e instanceof Error ? e.message : 'Connection failed';
			connecting = false;
		}
	}

	function connectWithUrl() {
		const trimmed = urlInput.trim();
		if (!trimmed) return;

		// Allow entering just a hostname — build the full WS URL
		let wsUrl: string;
		if (trimmed.startsWith('ws://') || trimmed.startsWith('wss://')) {
			try {
				new URL(trimmed);
			} catch {
				inputError = 'Invalid URL format';
				return;
			}
			wsUrl = trimmed;
		} else {
			// Treat as hostname (e.g. "macbook.tail1234.ts.net" or "192.168.1.50")
			wsUrl = `ws://${trimmed}:${WS_PORT}/ws`;
		}
		doConnect(wsUrl);
	}

	onMount(() => {
		// Auto-connect if URL already stored (e.g. from previous session)
		const existing = getStoredWsUrl();
		if (existing) {
			doConnect(existing);
			return;
		}

		// Auto-connect if accessed via Tailscale MagicDNS hostname
		const hostname = window.location.hostname;
		if (hostname.endsWith('.ts.net')) {
			doConnect(`ws://${hostname}:${WS_PORT}/ws`);
			return;
		}

		// Auto-connect if accessed via a non-localhost address (e.g. LAN IP served by the app)
		if (hostname !== 'localhost' && hostname !== '127.0.0.1' && hostname !== '::1' && hostname !== '') {
			doConnect(`ws://${hostname}:${WS_PORT}/ws`);
		}
	});
</script>

<div class="connection-screen">
	<div class="content">
		<div class="header">
			<div class="logo-box">
				<span class="logo-text">C9</span>
			</div>
			<h1 class="title">c9watch</h1>
			<p class="subtitle">Connect to your desktop session</p>
		</div>

		{#if connecting}
			<div class="connecting">
				<div class="spinner"></div>
				<span class="connecting-text">Connecting...</span>
				{#if inputError}
					<div class="error">{inputError}</div>
					<button class="back-btn" onclick={() => { connecting = false; inputError = ''; }}>Retry</button>
				{/if}
			</div>
		{:else}
			<div class="paste-view">
				<label class="input-label" for="host-input">Hostname or WebSocket URL</label>
				<input
					id="host-input"
					type="text"
					class="url-input"
					bind:value={urlInput}
					placeholder="e.g. macbook.tail1234.ts.net"
					autocomplete="off"
					autocapitalize="off"
					onkeydown={(e) => e.key === 'Enter' && connectWithUrl()}
				/>
				{#if inputError}
					<div class="error">{inputError}</div>
				{/if}
				<div class="hint">Enter your Tailscale hostname or IP address</div>
				<div class="paste-actions">
					<button class="connect-btn" onclick={connectWithUrl} disabled={!urlInput.trim()}>
						Connect
					</button>
				</div>
			</div>
		{/if}
	</div>
</div>

<style>
	.connection-screen {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100vh;
		width: 100vw;
		background: var(--bg-base);
		padding: var(--space-xl);
	}

	.content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-3xl);
		width: 100%;
		max-width: 360px;
	}

	.header {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
	}

	.logo-box {
		width: 56px;
		height: 56px;
		border: 1px solid var(--border-default);
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: var(--space-sm);
	}

	.logo-text {
		font-family: var(--font-pixel);
		font-size: 20px;
		font-weight: 700;
		color: var(--text-primary);
	}

	.title {
		font-family: var(--font-pixel);
		font-size: 24px;
		font-weight: 600;
		color: var(--text-primary);
		text-transform: uppercase;
		letter-spacing: 0.15em;
	}

	.subtitle {
		font-family: var(--font-mono);
		font-size: 13px;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	/* Paste view */
	.paste-view {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		width: 100%;
	}

	.input-label {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.url-input {
		width: 100%;
		padding: var(--space-md);
		border: 1px solid var(--border-default);
		background: var(--bg-card);
		color: var(--text-primary);
		font-family: var(--font-mono);
		font-size: 13px;
	}

	.url-input::placeholder {
		color: var(--text-muted);
	}

	.url-input:focus {
		outline: none;
		border-color: var(--text-secondary);
	}

	.paste-actions {
		display: flex;
		gap: var(--space-md);
		margin-top: var(--space-sm);
	}

	.back-btn {
		flex: 1;
		padding: var(--space-md);
		border: 1px solid var(--border-default);
		background: transparent;
		color: var(--text-secondary);
		font-family: var(--font-mono);
		font-size: 13px;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		cursor: pointer;
		min-height: 0;
		min-width: 0;
	}

	.back-btn:hover {
		border-color: var(--text-muted);
		color: var(--text-primary);
	}

	.connect-btn {
		flex: 1;
		padding: var(--space-md);
		border: 1px solid var(--text-primary);
		background: var(--text-primary);
		color: var(--bg-base);
		font-family: var(--font-mono);
		font-size: 13px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		cursor: pointer;
		min-height: 0;
		min-width: 0;
	}

	.connect-btn:hover {
		opacity: 0.9;
	}

	.connect-btn:disabled {
		opacity: 0.3;
		cursor: not-allowed;
	}

	.hint {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--text-muted);
	}

	.connecting {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
	}

	.spinner {
		width: 24px;
		height: 24px;
		border: 2px solid var(--border-default);
		border-top-color: var(--text-primary);
		animation: spin 0.8s linear infinite;
	}

	.connecting-text {
		font-family: var(--font-mono);
		font-size: 13px;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.error {
		font-family: var(--font-mono);
		font-size: 12px;
		color: var(--accent-red);
		text-align: center;
	}

	@media (max-width: 768px) {
		.connection-screen {
			padding: var(--space-lg);
		}
	}
</style>
