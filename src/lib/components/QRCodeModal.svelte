<script lang="ts">
	import { onMount } from 'svelte';
	import QRCode from 'qrcode';
	import { getServerInfo, type ServerInfo } from '$lib/api';

	let { onclose }: { onclose: () => void } = $props();

	let info = $state<ServerInfo | null>(null);
	let qrDataUrl = $state<string>('');
	let pageUrl = $state('');
	let error = $state<string>('');
	let copied = $state(false);

	onMount(async () => {
		try {
			info = await getServerInfo();
			if (info.tailscaleHostname) {
				const scheme = info.tls ? 'https' : 'http';
				pageUrl = `${scheme}://${info.tailscaleHostname}:${info.port}/`;
			} else {
				pageUrl = `http://${info.localIp}:${info.port}/`;
			}
			qrDataUrl = await QRCode.toDataURL(pageUrl, {
				width: 256,
				margin: 2,
				color: { dark: '#ffffff', light: '#000000' }
			});
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load server info';
		}
	});

	async function copyUrl() {
		if (!pageUrl) return;
		try {
			await navigator.clipboard.writeText(pageUrl);
		} catch {
			const el = document.createElement('textarea');
			el.value = pageUrl;
			document.body.appendChild(el);
			el.select();
			document.execCommand('copy');
			document.body.removeChild(el);
		}
		copied = true;
		setTimeout(() => (copied = false), 2000);
	}

	function handleBackdrop(e: MouseEvent) {
		if (e.target === e.currentTarget) onclose();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onclose();
	}
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="backdrop" onclick={handleBackdrop} role="dialog" aria-modal="true" aria-label="Connect Mobile" tabindex="-1">
	<div class="modal">
		<div class="modal-header">
			<span class="modal-title">Connect Mobile</span>
			<button class="close-btn" onclick={onclose} aria-label="Close">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<line x1="18" y1="6" x2="6" y2="18" />
					<line x1="6" y1="6" x2="18" y2="18" />
				</svg>
			</button>
		</div>

		{#if error}
			<div class="error">{error}</div>
		{:else if !info}
			<div class="loading">Loading server info...</div>
		{:else}
			{#if info.tailscaleHostname}
				<div class="qr-container">
					<img src={qrDataUrl} alt="QR Code" class="qr-image" />
				</div>

				<div class="info-section">
					<span class="info-label">URL</span>
					<button class="url-box" onclick={copyUrl} title="Click to copy">
						<code class="url-text">{pageUrl}</code>
						<span class="copy-hint">{copied ? 'COPIED' : 'COPY'}</span>
					</button>
				</div>

				<div class="instructions">
					<p>Scan QR with phone camera to open in browser. Both devices must be on your Tailscale network.</p>
				</div>
			{:else}
				<div class="no-tailscale">
					<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
						<circle cx="12" cy="12" r="10" />
						<line x1="12" y1="8" x2="12" y2="12" />
						<line x1="12" y1="16" x2="12.01" y2="16" />
					</svg>
					<p>Tailscale not detected</p>
					<p class="no-tailscale-detail">Remote access requires Tailscale to be running. Start Tailscale and relaunch c9watch to enable mobile access.</p>
				</div>

				<div class="info-section">
					<span class="info-label">Local URL (same machine only)</span>
					<button class="url-box" onclick={copyUrl} title="Click to copy">
						<code class="url-text">{pageUrl}</code>
						<span class="copy-hint">{copied ? 'COPIED' : 'COPY'}</span>
					</button>
				</div>
			{/if}
		{/if}
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.85);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 9999;
		animation: fade-in 150ms ease;
	}

	.modal {
		background: var(--bg-card);
		border: 1px solid var(--border-default);
		width: 380px;
		max-width: 90vw;
		padding: var(--space-xl);
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		animation: scale-in 150ms ease;
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.modal-title {
		font-family: var(--font-pixel);
		font-size: 16px;
		font-weight: 600;
		color: var(--text-primary);
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.close-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		color: var(--text-muted);
		border: 1px solid transparent;
		background: transparent;
		cursor: pointer;
	}

	.close-btn:hover {
		color: var(--text-primary);
		border-color: var(--border-default);
	}

	.qr-container {
		display: flex;
		justify-content: center;
		padding: var(--space-md);
		border: 1px solid var(--border-muted);
		background: #000;
	}

	.qr-image {
		width: 256px;
		height: 256px;
		image-rendering: pixelated;
	}

	.info-section {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.info-label {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.url-box {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-sm);
		padding: var(--space-sm) var(--space-md);
		border: 1px solid var(--border-default);
		background: var(--bg-base);
		cursor: pointer;
		text-align: left;
		min-height: 0;
		min-width: 0;
	}

	.url-box:hover {
		border-color: var(--text-muted);
	}

	.url-text {
		font-family: var(--font-mono);
		font-size: 12px;
		color: var(--text-secondary);
		word-break: break-all;
	}

	.copy-hint {
		font-family: var(--font-mono);
		font-size: 10px;
		color: var(--accent-green);
		letter-spacing: 0.1em;
		flex-shrink: 0;
	}

	.no-tailscale {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-xl) 0;
		color: var(--text-muted);
	}

	.no-tailscale p {
		font-family: var(--font-mono);
		font-size: 14px;
		font-weight: 600;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.no-tailscale-detail {
		font-size: 12px !important;
		font-weight: 400 !important;
		text-transform: none !important;
		letter-spacing: normal !important;
		text-align: center;
		line-height: 1.5;
	}

	.instructions {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.instructions p {
		font-family: var(--font-mono);
		font-size: 12px;
		color: var(--text-muted);
		line-height: 1.6;
	}

	.loading, .error {
		font-family: var(--font-mono);
		font-size: 13px;
		color: var(--text-muted);
		text-align: center;
		padding: var(--space-3xl) 0;
	}

	.error {
		color: var(--accent-red);
	}
</style>
