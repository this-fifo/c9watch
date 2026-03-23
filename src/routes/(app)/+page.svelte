<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { browser } from '$app/environment';
	import {
		sortedSessions,
		expandedSessionId,
		currentConversation,
		statusSummary
	} from '$lib/stores/sessions';
	import {
		getConversation,
		stopSession,
		openSession,
		getSessionHistory,
		deepSearchSessions
	} from '$lib/api';
	import { isTauri } from '$lib/ws';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import ExpandedCardOverlay from '$lib/components/ExpandedCardOverlay.svelte';
	import HistoryCardOverlay from '$lib/components/HistoryCardOverlay.svelte';
	import ToastNotifications from '$lib/components/ToastNotifications.svelte';
	import QRCodeModal from '$lib/components/QRCodeModal.svelte';
	import ConnectionScreen from '$lib/components/ConnectionScreen.svelte';
	import type { Session, HistoryEntry, Conversation } from '$lib/types';
	import { SessionStatus } from '$lib/types';
	import CostTracker from '$lib/components/CostTracker.svelte';
	import MemoryViewer from '$lib/components/MemoryViewer.svelte';
	import FdaBanner from '$lib/components/FdaBanner.svelte';
	import DebugConsole from '$lib/components/DebugConsole.svelte';
	import type { DetectionDiagnostics } from '$lib/types';

	let showQRModal = $state(false);
	let needsConnection = $state(!isTauri());

	let sessions = $derived($sortedSessions);
	let activeSessionIds = $derived(new Set(sessions.map((s) => s.id)));
	let summary = $derived($statusSummary);
	let expandedId = $derived($expandedSessionId);
	let conversation = $derived($currentConversation);

	let activeTab = $state<'sessions' | 'cost' | 'memory'>('sessions');
	let fdaLikelyNeeded = $state(false);
	let showDebugConsole = $state(false);
	let showRenameHint = $state(false);

	// History data
	let allHistoryEntries = $state<HistoryEntry[]>([]);
	let historyLoading = $state(true);

	// Search & view controls
	let query = $state('');
	let sortOrder = $state<'newest' | 'oldest'>('newest');
	let groupByProject = $state(true);
	let collapsedProjects = $state<Set<string>>(new Set());

	// Deep search
	let deepSearching = $state(false);
	let deepSearchResults = $state<Map<string, string> | null>(null);

	// History conversation viewer
	let selectedHistoryEntry = $state<HistoryEntry | null>(null);
	let historyConversation = $state<Conversation | null>(null);

	// Fullscreen detection
	let isFullscreen = $state(false);

	// ── Unified session type ─────────────────────────────────────────
	interface UnifiedSession {
		id: string;
		title: string;
		projectPath: string;
		projectName: string;
		timestamp: number;
		isActive: boolean;
		status?: SessionStatus;
		session?: Session;
		historyEntry?: HistoryEntry;
	}

	// ── Build unified list ───────────────────────────────────────────
	let unified = $derived.by(() => {
		const items: UnifiedSession[] = [];
		const seenIds = new Set<string>();

		for (const s of sessions) {
			seenIds.add(s.id);
			const parts = s.projectPath.split(/[/\\]/);
			items.push({
				id: s.id,
				title: s.customTitle || s.summary || s.firstPrompt || '(no title)',
				projectPath: s.projectPath,
				projectName: parts.filter(Boolean).pop() || s.projectPath,
				timestamp: new Date(s.modified).getTime(),
				isActive: true,
				status: s.status,
				session: s
			});
		}

		for (const h of allHistoryEntries) {
			if (seenIds.has(h.sessionId)) continue;
			items.push({
				id: h.sessionId,
				title: h.customTitle || h.display || '(no prompt)',
				projectPath: h.project,
				projectName: h.projectName,
				timestamp: h.timestamp,
				isActive: false,
				historyEntry: h
			});
		}

		return items;
	});

	// ── Filter & sort ────────────────────────────────────────────────
	let filtered = $derived.by(() => {
		let items = unified;

		if (query.trim()) {
			const words = query.toLowerCase().split(/\s+/).filter(Boolean);
			items = items.filter((item) => {
				const title = item.title.toLowerCase();
				const project = item.projectName.toLowerCase();
				return words.every((w) => title.includes(w) || project.includes(w));
			});

			if (deepSearchResults !== null) {
				const metaIds = new Set(items.map((i) => i.id));
				const deepOnly = unified.filter(
					(i) => deepSearchResults!.has(i.id) && !metaIds.has(i.id)
				);
				items = [...items, ...deepOnly];
			}
		}

		return [...items].sort((a, b) => {
			// Active sessions always first
			if (a.isActive !== b.isActive) return a.isActive ? -1 : 1;
			// Among active: priority sort
			if (a.isActive && b.isActive && a.status && b.status) {
				const priority: Record<SessionStatus, number> = {
					[SessionStatus.NeedsAttention]: 0,
					[SessionStatus.WaitingForInput]: 1,
					[SessionStatus.Working]: 2,
					[SessionStatus.Connecting]: 3
				};
				const pa = priority[a.status] ?? 4;
				const pb = priority[b.status] ?? 4;
				if (pa !== pb) return pa - pb;
			}
			return sortOrder === 'newest' ? b.timestamp - a.timestamp : a.timestamp - b.timestamp;
		});
	});

	// ── Grouping ─────────────────────────────────────────────────────
	let groups = $derived.by(() => {
		if (!groupByProject) return null;

		const map = new Map<string, { path: string; name: string; items: UnifiedSession[] }>();
		for (const item of filtered) {
			if (!map.has(item.projectPath)) {
				map.set(item.projectPath, { path: item.projectPath, name: item.projectName, items: [] });
			}
			map.get(item.projectPath)!.items.push(item);
		}

		return [...map.values()].sort((a, b) => {
			const aHasActive = a.items.some((i) => i.isActive);
			const bHasActive = b.items.some((i) => i.isActive);
			if (aHasActive !== bHasActive) return aHasActive ? -1 : 1;
			const aTime = Math.max(...a.items.map((i) => i.timestamp));
			const bTime = Math.max(...b.items.map((i) => i.timestamp));
			return bTime - aTime;
		});
	});

	// ── Collapse state ───────────────────────────────────────────────
	$effect(() => {
		if (!groupByProject) collapsedProjects = new Set();
	});

	let allCollapsed = $derived(
		groups !== null && groups.length > 0 && groups.every((g) => collapsedProjects.has(g.path))
	);

	function toggleProjectCollapse(path: string) {
		const next = new Set(collapsedProjects);
		if (next.has(path)) next.delete(path);
		else next.add(path);
		collapsedProjects = next;
	}

	// ── Deep search ──────────────────────────────────────────────────
	$effect(() => {
		const q = query;
		if (!q.trim()) {
			deepSearchResults = null;
			deepSearching = false;
			return;
		}
		deepSearchResults = null;
		let cancelled = false;
		const timer = setTimeout(async () => {
			deepSearching = true;
			try {
				const hits = await deepSearchSessions(q);
				if (!cancelled) deepSearchResults = new Map(hits.map((h) => [h.sessionId, h.snippet]));
			} catch {
				// ignore
			} finally {
				if (!cancelled) deepSearching = false;
			}
		}, 300);
		return () => {
			cancelled = true;
			clearTimeout(timer);
		};
	});

	// ── Lifecycle ────────────────────────────────────────────────────
	onMount(() => {
		if (browser) {
			const savedSort = localStorage.getItem('sessionSort');
			if (savedSort === 'newest' || savedSort === 'oldest') sortOrder = savedSort;
			const savedGroup = localStorage.getItem('sessionGroup');
			if (savedGroup === 'false') groupByProject = false;
		}

		// Load history (async, fire-and-forget)
		getSessionHistory()
			.then((entries) => (allHistoryEntries = entries))
			.catch(() => {})
			.finally(() => (historyLoading = false));

		if (!isTauri()) return;

		let unlisten: (() => void) | null = null;
		let timer: ReturnType<typeof setTimeout> | null = null;

		(async () => {
			const { getCurrentWindow } = await import('@tauri-apps/api/window');
			const win = getCurrentWindow();
			isFullscreen = await win.isFullscreen();
			unlisten = await win.onResized(async () => {
				if (timer) clearTimeout(timer);
				timer = setTimeout(async () => {
					isFullscreen = await win.isFullscreen();
				}, 150);
			});
		})();

		import('@tauri-apps/api/event').then(({ listen }) => {
			listen<DetectionDiagnostics>('diagnostic-update', (event) => {
				fdaLikelyNeeded = event.payload.fdaLikelyNeeded;
			});
		});

		return () => {
			unlisten?.();
			if (timer) clearTimeout(timer);
		};
	});

	$effect(() => {
		if (browser) localStorage.setItem('sessionSort', sortOrder);
	});

	$effect(() => {
		if (browser) localStorage.setItem('sessionGroup', String(groupByProject));
	});

	// ── Active session expand ────────────────────────────────────────
	let expandedSession = $derived(sessions.find((s) => s.id === expandedId) || null);

	$effect(() => {
		if (expandedId) {
			getConversation(expandedId)
				.then((conv) => currentConversation.set(conv))
				.catch(() => currentConversation.set(null));
		} else {
			currentConversation.set(null);
		}
	});

	function handleExpand(session: Session) {
		expandedSessionId.set(session.id);
	}

	function handleClose() {
		expandedSessionId.set(null);
	}

	async function handleStop(pid: number) {
		try {
			await stopSession(pid);
		} catch (error) {
			console.error('Failed to stop session:', error);
		}
	}

	async function handleOpen(pid: number, projectPath: string) {
		try {
			await openSession(pid, projectPath);
		} catch (error) {
			console.error('Failed to open session:', error);
		}
	}

	// ── History conversation ─────────────────────────────────────────
	async function handleSelectHistoryEntry(entry: HistoryEntry) {
		selectedHistoryEntry = entry;
		historyConversation = null;
		try {
			historyConversation = await getConversation(entry.sessionId);
		} catch {
			// ignore
		}
	}

	// ── Row click ────────────────────────────────────────────────────
	function handleRowClick(item: UnifiedSession) {
		if (item.isActive && item.session) {
			handleExpand(item.session);
		} else if (item.historyEntry) {
			handleSelectHistoryEntry(item.historyEntry);
		}
	}

	// ── Helpers ──────────────────────────────────────────────────────
	function relativeTime(ms: number): string {
		const diff = Date.now() - ms;
		const mins = Math.floor(diff / 60_000);
		if (mins < 1) return 'just now';
		if (mins < 60) return `${mins}m ago`;
		const hours = Math.floor(mins / 60);
		if (hours < 24) return `${hours}h ago`;
		const days = Math.floor(hours / 24);
		if (days === 1) return 'yesterday';
		if (days < 7) return `${days}d ago`;
		return new Date(ms).toLocaleDateString();
	}

	function highlight(text: string, kw: string): string {
		if (!kw.trim()) return escapeHtml(text);
		const words = kw.split(/\s+/).filter(Boolean);
		if (words.length === 0) return escapeHtml(text);
		const escaped = escapeHtml(text);
		const pattern = words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|');
		return escaped.replace(new RegExp(pattern, 'gi'), (m) => `<mark>${m}</mark>`);
	}

	function escapeHtml(s: string): string {
		return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
	}

	function getStatusLabel(status: SessionStatus): string {
		switch (status) {
			case SessionStatus.Working:
				return 'WORKING';
			case SessionStatus.NeedsAttention:
				return 'ACTION';
			case SessionStatus.WaitingForInput:
				return 'READY';
			case SessionStatus.Connecting:
				return 'CONNECTING';
			default:
				return '';
		}
	}

	function getStatusClass(status: SessionStatus): string {
		switch (status) {
			case SessionStatus.Working:
				return 'working';
			case SessionStatus.NeedsAttention:
				return 'attention';
			case SessionStatus.WaitingForInput:
				return 'idle';
			case SessionStatus.Connecting:
				return 'connecting';
			default:
				return '';
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		const tag = (e.target as HTMLElement)?.tagName;
		if (tag === 'INPUT' || tag === 'TEXTAREA') return;

		if ((e.key === 'd' || e.key === 'D') && e.shiftKey && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			showDebugConsole = !showDebugConsole;
			return;
		}
		if (e.key >= '1' && e.key <= '9' && !expandedId) {
			const index = parseInt(e.key) - 1;
			if (index < sessions.length) {
				handleExpand(sessions[index]);
			}
		}
		if (e.key === 'Tab' && !expandedId) {
			const needsAction = sessions.filter(
				(s) =>
					s.status === SessionStatus.NeedsAttention ||
					s.status === SessionStatus.WaitingForInput
			);
			if (needsAction.length > 0) {
				e.preventDefault();
				handleExpand(needsAction[0]);
			}
		}
	}
</script>

<svelte:window on:keydown={handleKeydown} />

{#if needsConnection}
	<ConnectionScreen onconnected={() => (needsConnection = false)} />
{:else}
	<div class="dashboard">
		<FdaBanner {fdaLikelyNeeded} />
		<div class="tab-bar" class:fullscreen={isFullscreen} data-tauri-drag-region>
			<button
				class="tab-btn"
				class:active={activeTab === 'sessions'}
				onclick={() => (activeTab = 'sessions')}
			>
				<span class="tab-icon">■</span>
				<span class="tab-label">SESSIONS</span>
			</button>
			<button
				class="tab-btn"
				class:active={activeTab === 'cost'}
				onclick={() => (activeTab = 'cost')}
			>
				<span class="tab-icon">$</span>
				<span class="tab-label">COST</span>
			</button>
			<button
				class="tab-btn"
				class:active={activeTab === 'memory'}
				onclick={() => (activeTab = 'memory')}
			>
				<span class="tab-icon">◆</span>
				<span class="tab-label">MEMORY</span>
			</button>
			<div class="tab-drag-region" data-tauri-drag-region>
				{#if !isFullscreen}
					<span class="drag-dots" transition:fade={{ duration: 250 }}>⠿ ⠿ ⠿</span>
				{/if}
			</div>
		</div>

		{#if activeTab === 'cost'}
			<main class="grid-container tab-main">
				<CostTracker />
			</main>
		{:else if activeTab === 'memory'}
			<main class="grid-container tab-main">
				<MemoryViewer />
			</main>
		{:else}
			<main class="grid-container tab-main">
				<div class="sessions-container">
					<div class="controls">
						<div class="section-header">
							<span class="section-title">Sessions</span>
							<span class="section-count">{filtered.length}</span>
							{#if isTauri()}
								<button
									class="mobile-connect-btn"
									onclick={() => (showQRModal = true)}
									title="Connect Mobile Device"
								>
									<svg
										width="14"
										height="14"
										viewBox="0 0 24 24"
										fill="none"
										stroke="currentColor"
										stroke-width="2"
										stroke-linecap="round"
										stroke-linejoin="round"
									>
										<rect x="5" y="2" width="14" height="20" rx="2" ry="2" />
										<line x1="12" y1="18" x2="12.01" y2="18" />
									</svg>
									<span class="mobile-label">MOBILE</span>
								</button>
							{/if}
						</div>

						{#if sessions.length > 0}
							<StatusBar total={sessions.length} {summary} />
						{/if}

						<div class="search-row">
							<input
								class="search-input"
								type="text"
								placeholder="Search sessions..."
								bind:value={query}
							/>
						</div>

						<div class="options-row">
							<div class="sort-group">
								<button
									class="option-btn"
									class:active={sortOrder === 'newest'}
									onclick={() => (sortOrder = 'newest')}>NEWEST</button
								>
								<button
									class="option-btn"
									class:active={sortOrder === 'oldest'}
									onclick={() => (sortOrder = 'oldest')}>OLDEST</button
								>
							</div>

							<div class="sort-group">
								<button
									class="option-btn"
									class:active={!groupByProject}
									onclick={() => (groupByProject = false)}>FLAT</button
								>
								<button
									class="option-btn"
									class:active={groupByProject}
									onclick={() => (groupByProject = true)}>BY PROJECT</button
								>
							</div>

							{#if groupByProject}
								<div class="sort-group">
									<button
										class="option-btn"
										onclick={() => {
											if (allCollapsed) {
												collapsedProjects = new Set();
											} else if (groups) {
												collapsedProjects = new Set(groups.map((g) => g.path));
											}
										}}
									>
										{allCollapsed ? 'EXPAND ALL' : 'COLLAPSE ALL'}
									</button>
								</div>
							{/if}
						</div>
					</div>

					{#if deepSearching}
						<div class="searching-indicator">Searching...</div>
					{/if}

					<div class="list-area">
						{#if historyLoading && sessions.length === 0}
							<div class="state-msg">Loading sessions...</div>
						{:else if filtered.length === 0}
							{#if sessions.length === 0 && !query.trim()}
								<div class="empty-state">
									<div class="empty-visual">
										<div class="empty-orb">
											<div class="orb-core"></div>
										</div>
									</div>
									<div class="empty-content">
										<h2>No Active Sessions</h2>
										<p>Start a Claude Code session in your terminal or IDE</p>
										<div class="empty-hint">
											<span class="hint-icon">
												<svg
													width="14"
													height="14"
													viewBox="0 0 24 24"
													fill="none"
													stroke="currentColor"
													stroke-width="2"
												>
													<circle cx="12" cy="12" r="10" />
													<path d="M12 16v-4" />
													<path d="M12 8h.01" />
												</svg>
											</span>
											Sessions are detected automatically
										</div>
									</div>
								</div>
							{:else}
								<div class="state-msg">No sessions found.</div>
							{/if}
						{:else if groupByProject && groups}
							{#each groups as group (group.path)}
								<div class="project-group">
									<!-- svelte-ignore a11y_click_events_have_key_events -->
									<!-- svelte-ignore a11y_no_static_element_interactions -->
									<div
										class="group-header"
										class:has-active={group.items.some((i) => i.isActive)}
										onclick={() => toggleProjectCollapse(group.path)}
										role="button"
										tabindex="0"
										aria-label={collapsedProjects.has(group.path)
											? 'Expand group'
											: 'Collapse group'}
									>
										<span class="collapse-toggle" aria-hidden="true"
											>{collapsedProjects.has(group.path) ? '▶' : '▼'}</span
										>
										<span class="group-name">{group.name}</span>
										<span class="group-count">{group.items.length}</span>
										{#if group.items.some((i) => i.isActive)}
											{@const liveCount = group.items.filter((i) => i.isActive).length}
											<span class="group-live-badge">{liveCount} LIVE</span>
										{/if}
									</div>
									{#if !collapsedProjects.has(group.path)}
										{#each group.items as item (item.id)}
											{@const snippet = query.trim()
												? (deepSearchResults?.get(item.id) ?? null)
												: null}
											<button
												class="session-row"
												class:active-session={item.isActive}
												class:has-snippet={!!snippet}
												onclick={() => handleRowClick(item)}
											>
												{#if item.isActive && item.status}
													<span
														class="status-dot {getStatusClass(item.status)}"
													></span>
												{:else}
													<span class="status-dot past"></span>
												{/if}
												<div class="row-content">
													<div class="row-top">
														{#if item.isActive && item.status}
															<span
																class="status-badge {getStatusClass(item.status)}"
																>{getStatusLabel(item.status)}</span
															>
														{/if}
														<span class="row-meta">
															<span class="row-time"
																>{relativeTime(item.timestamp)}</span
															>
														</span>
													</div>
													<span class="row-prompt"
														>{@html highlight(
															snippet ?? item.title,
															query
														)}</span
													>
												</div>
											</button>
										{/each}
									{/if}
								</div>
							{/each}
						{:else}
							{#each filtered as item (item.id)}
								{@const snippet = query.trim()
									? (deepSearchResults?.get(item.id) ?? null)
									: null}
								<button
									class="session-row"
									class:active-session={item.isActive}
									class:has-snippet={!!snippet}
									onclick={() => handleRowClick(item)}
								>
									{#if item.isActive && item.status}
										<span class="status-dot {getStatusClass(item.status)}"
										></span>
									{:else}
										<span class="status-dot past"></span>
									{/if}
									<div class="row-content">
										<div class="row-top">
											<span class="row-project">{item.projectName}</span>
											<span class="row-meta">
												{#if item.isActive && item.status}
													<span
														class="status-badge {getStatusClass(item.status)}"
														>{getStatusLabel(item.status)}</span
													>
												{/if}
												<span class="row-time"
													>{relativeTime(item.timestamp)}</span
												>
											</span>
										</div>
										<span class="row-prompt"
											>{@html highlight(
												snippet ?? item.title,
												query
											)}</span
										>
									</div>
								</button>
							{/each}
						{/if}
					</div>
				</div>
			</main>
		{/if}

		<!-- Active session overlay -->
		{#if expandedSession}
			<ExpandedCardOverlay
				session={expandedSession}
				{conversation}
				onclose={handleClose}
				onstop={() => handleStop(expandedSession.pid)}
				onopen={() => handleOpen(expandedSession.pid, expandedSession.projectPath)}
			/>
		{/if}

		<!-- History session overlay -->
		{#if selectedHistoryEntry}
			<HistoryCardOverlay
				entry={selectedHistoryEntry}
				conversation={historyConversation}
				searchQuery={query.trim() && deepSearchResults?.has(selectedHistoryEntry.sessionId)
					? query.trim()
					: undefined}
				onclose={() => {
					selectedHistoryEntry = null;
					historyConversation = null;
				}}
			/>
		{/if}

		{#if showQRModal}
			<QRCodeModal onclose={() => (showQRModal = false)} />
		{/if}

		<ToastNotifications />
		<DebugConsole
			visible={showDebugConsole}
			onclose={() => (showDebugConsole = false)}
		/>

		{#if showRenameHint}
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="rename-hint-backdrop"
				transition:fade={{ duration: 150 }}
				onclick={() => (showRenameHint = false)}
			>
				<div class="rename-hint-modal" onclick={(e) => e.stopPropagation()}>
					<div class="rename-hint-header">
						<span class="rename-hint-icon">
							<svg
								width="16"
								height="16"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
							>
								<path
									d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"
								/>
								<path
									d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"
								/>
							</svg>
						</span>
						<span class="rename-hint-title">Rename Session</span>
					</div>
					<p class="rename-hint-text">
						Use the <code>/rename</code> command in your Claude Code session to rename
						it.
					</p>
					<div class="rename-hint-example">/rename my-task-name</div>
					<p class="rename-hint-sub">
						The new name will automatically appear in c9watch.
					</p>
					<button
						type="button"
						class="rename-hint-close"
						onclick={() => (showRenameHint = false)}>GOT IT</button
					>
				</div>
			</div>
		{/if}
	</div>
{/if}

<style>
	.dashboard {
		display: flex;
		flex-direction: column;
		height: 100vh;
		width: 100vw;
		overflow: hidden;
		background: var(--bg-base);
	}

	/* ── Tab bar ───────────────────────────────────────────────────── */
	.tab-bar {
		height: 28px;
		width: 100%;
		flex-shrink: 0;
		display: flex;
		align-items: stretch;
		background: transparent;
		z-index: 1000;
		position: relative;
		padding: 0 var(--space-md) 0 80px;
		transition: padding-left 0.35s ease;
		-webkit-app-region: drag;
	}

	.tab-drag-region {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		-webkit-app-region: drag;
		cursor: grab;
	}

	.drag-dots {
		position: absolute;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		font-size: 14px;
		letter-spacing: 3px;
		color: var(--text-muted);
		opacity: 0.5;
		user-select: none;
		line-height: 1;
		pointer-events: none;
		transition: opacity var(--transition-fast);
	}

	.tab-drag-region:hover .drag-dots {
		opacity: 0.85;
	}

	.tab-btn {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		padding: 0 var(--space-md);
		background: transparent;
		border: none;
		border-bottom: 2px solid transparent;
		color: var(--text-muted);
		cursor: pointer;
		font-family: var(--font-sans);
		font-size: 10px;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		transition: color var(--transition-fast);
		-webkit-app-region: no-drag;
	}

	.tab-btn:hover {
		color: var(--text-secondary);
	}

	.tab-btn.active {
		color: var(--text-primary);
		border-bottom-color: var(--text-primary);
	}

	.tab-icon {
		font-size: 8px;
	}

	.tab-label {
		font-size: 10px;
	}

	/* ── Main container ────────────────────────────────────────────── */
	.grid-container {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-xl);
	}

	.tab-main {
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}

	.sessions-container {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	/* ── Controls ──────────────────────────────────────────────────── */
	.controls {
		flex-shrink: 0;
		padding: 0 0 var(--space-md);
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		border-bottom: 1px solid var(--border-default);
	}

	.section-header {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-bottom: var(--space-md);
		border-bottom: 1px solid var(--text-primary);
		margin-bottom: var(--space-sm);
	}

	.section-title {
		font-family: var(--font-sans);
		font-size: 22px;
		font-weight: 700;
		color: var(--text-primary);
		letter-spacing: 0.02em;
		line-height: 1;
	}

	.section-count {
		font-family: var(--font-mono);
		font-size: 16px;
		font-weight: 500;
		line-height: 1;
		color: var(--text-secondary);
	}

	.mobile-connect-btn {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		margin-left: auto;
		padding: 2px var(--space-sm);
		border: 1px solid transparent;
		background: transparent;
		color: var(--accent-blue);
		cursor: pointer;
	}

	.mobile-connect-btn:hover {
		background: rgba(0, 112, 243, 0.1);
	}

	.mobile-label {
		font-family: var(--font-sans);
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.05em;
		text-transform: uppercase;
	}

	.search-row {
		display: flex;
	}

	.search-input {
		width: 100%;
		background: var(--bg-elevated);
		border: 1px solid var(--border-default);
		color: var(--text-primary);
		font-family: var(--font-mono);
		font-size: 13px;
		padding: var(--space-sm) var(--space-md);
		outline: none;
	}

	.search-input:focus {
		border-color: var(--border-focus);
	}

	.search-input::placeholder {
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.options-row {
		display: flex;
		gap: var(--space-md);
	}

	.sort-group {
		display: flex;
		border: 1px solid var(--border-default);
	}

	.option-btn {
		font-family: var(--font-sans);
		font-size: 10px;
		font-weight: 600;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		padding: 4px var(--space-sm);
		background: transparent;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
	}

	.option-btn.active {
		background: rgba(255, 255, 255, 0.1);
		color: var(--text-primary);
	}

	.searching-indicator {
		flex-shrink: 0;
		padding: var(--space-xs) 0;
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	/* ── List ──────────────────────────────────────────────────────── */
	.list-area {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-md) 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.state-msg {
		font-family: var(--font-mono);
		font-size: 13px;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		padding: var(--space-xl) 0;
		text-align: center;
	}

	/* ── Project groups ────────────────────────────────────────────── */
	.project-group {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		margin-bottom: var(--space-xl);
	}

	.group-header {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-bottom: var(--space-sm);
		border-bottom: 1px solid var(--border-default);
		margin-bottom: var(--space-sm);
		cursor: pointer;
	}

	.group-header:hover .group-name {
		color: var(--text-primary);
	}

	.collapse-toggle {
		color: var(--text-muted);
		font-family: var(--font-mono);
		font-size: 11px;
		line-height: 1;
		flex-shrink: 0;
	}

	.group-name {
		font-family: var(--font-sans);
		font-size: 16px;
		font-weight: 600;
		color: var(--text-primary);
		letter-spacing: 0.02em;
	}

	.group-count {
		font-family: var(--font-mono);
		font-size: 13px;
		color: var(--text-muted);
	}

	.group-live-badge {
		font-family: var(--font-sans);
		font-size: 9px;
		font-weight: 700;
		color: var(--accent-green);
		background: color-mix(in srgb, var(--accent-green) 10%, transparent);
		padding: 1px 6px;
		border: 1px solid color-mix(in srgb, var(--accent-green) 30%, transparent);
		letter-spacing: 0.08em;
		line-height: 1.2;
		text-transform: uppercase;
	}

	/* ── Session rows ──────────────────────────────────────────────── */
	.session-row {
		width: 100%;
		text-align: left;
		background: var(--bg-card);
		border: 1px solid var(--border-muted);
		padding: var(--space-md);
		cursor: pointer;
		display: flex;
		flex-direction: row;
		align-items: flex-start;
		gap: var(--space-md);
		transition: border-color var(--transition-fast), background var(--transition-fast);
	}

	.session-row:hover {
		border-color: var(--border-default);
		background: var(--bg-card-hover);
	}

	.session-row.active-session {
		background: rgba(255, 255, 255, 0.03);
		border-left: 3px solid var(--border-default);
	}

	.session-row.active-session:hover {
		background: rgba(255, 255, 255, 0.06);
	}

	/* ── Status dot ────────────────────────────────────────────────── */
	.status-dot {
		width: 8px;
		height: 8px;
		flex-shrink: 0;
		margin-top: 5px;
	}

	.status-dot.working {
		background: var(--status-working);
		animation: pulse-glow 2s ease-in-out infinite;
	}

	.status-dot.attention {
		background: var(--status-permission);
		animation: pulse-glow 1s ease-in-out infinite;
	}

	.status-dot.idle {
		background: var(--status-input);
	}

	.status-dot.connecting {
		background: var(--status-connecting);
		animation: pulse-glow 1.5s ease-in-out infinite;
	}

	.status-dot.past {
		background: var(--border-default);
	}

	/* ── Row content ───────────────────────────────────────────────── */
	.row-content {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.row-top {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
	}

	.row-project {
		font-family: var(--font-sans);
		font-size: 13px;
		font-weight: 600;
		color: var(--text-primary);
		letter-spacing: 0.02em;
	}

	.row-meta {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		flex-shrink: 0;
		margin-left: auto;
	}

	.row-time {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--text-muted);
	}

	.status-badge {
		font-family: var(--font-sans);
		font-size: 9px;
		font-weight: 700;
		padding: 1px 5px;
		border: 1px solid;
		letter-spacing: 0.08em;
		line-height: 1;
		text-transform: uppercase;
		flex-shrink: 0;
	}

	.status-badge.working {
		color: var(--status-working);
		border-color: color-mix(in srgb, var(--status-working) 40%, transparent);
		background: color-mix(in srgb, var(--status-working) 10%, transparent);
	}

	.status-badge.attention {
		color: var(--status-permission);
		border-color: color-mix(in srgb, var(--status-permission) 40%, transparent);
		background: color-mix(in srgb, var(--status-permission) 10%, transparent);
	}

	.status-badge.idle {
		color: var(--status-input);
		border-color: color-mix(in srgb, var(--status-input) 40%, transparent);
		background: color-mix(in srgb, var(--status-input) 10%, transparent);
	}

	.status-badge.connecting {
		color: var(--status-connecting);
		border-color: color-mix(in srgb, var(--status-connecting) 40%, transparent);
		background: color-mix(in srgb, var(--status-connecting) 10%, transparent);
	}

	.row-prompt {
		font-family: var(--font-mono);
		font-size: 13px;
		color: var(--text-secondary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.session-row.has-snippet .row-prompt {
		white-space: normal;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}

	.row-prompt :global(mark) {
		background: transparent;
		color: var(--accent-amber);
		font-weight: 600;
	}

	/* ── Empty state ───────────────────────────────────────────────── */
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		gap: var(--space-3xl);
		padding: var(--space-3xl) 0;
	}

	.empty-visual {
		position: relative;
		width: 80px;
		height: 80px;
		border: 1px solid var(--border-default);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.empty-orb {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.orb-core {
		width: 8px;
		height: 8px;
		background: var(--text-muted);
		animation: pulse-glow 2s linear infinite;
	}

	.empty-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
	}

	.empty-content h2 {
		font-family: var(--font-sans);
		font-size: 18px;
		font-weight: 700;
		color: var(--text-primary);
		letter-spacing: 0.02em;
	}

	.empty-content p {
		font-family: var(--font-mono);
		font-size: 14px;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.empty-hint {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		margin-top: var(--space-md);
		padding: var(--space-sm) var(--space-lg);
		border: 1px solid var(--border-default);
		font-family: var(--font-mono);
		font-size: 13px;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.hint-icon {
		display: flex;
		color: var(--text-muted);
	}

	/* ── Fullscreen ────────────────────────────────────────────────── */
	.tab-bar.fullscreen {
		padding-left: var(--space-xl);
	}

	/* ── Mobile ────────────────────────────────────────────────────── */
	@media (max-width: 768px) {
		.tab-bar {
			height: 28px;
		}

		.tab-btn {
			display: none;
		}

		.grid-container {
			padding: var(--space-md);
		}

		.options-row {
			flex-wrap: wrap;
		}
	}

	/* ── Rename hint modal ─────────────────────────────────────────── */
	.rename-hint-backdrop {
		position: fixed;
		inset: 0;
		background: var(--bg-overlay);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 2000;
	}

	.rename-hint-modal {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-2xl);
		background: var(--bg-card);
		border: 1px solid var(--border-default);
		box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5);
		max-width: 380px;
		width: 100%;
	}

	.rename-hint-header {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.rename-hint-icon {
		color: var(--text-muted);
	}

	.rename-hint-title {
		font-family: var(--font-sans);
		font-size: 16px;
		font-weight: 700;
		color: var(--text-primary);
		letter-spacing: 0.02em;
	}

	.rename-hint-text {
		font-family: var(--font-mono);
		font-size: 13px;
		color: var(--text-secondary);
		text-align: center;
		margin: 0;
		line-height: 1.5;
	}

	.rename-hint-text code {
		font-family: var(--font-mono);
		color: var(--text-primary);
		background: var(--bg-elevated);
		padding: 2px 6px;
		border: 1px solid var(--border-default);
	}

	.rename-hint-example {
		font-family: var(--font-mono);
		font-size: 14px;
		color: var(--status-input);
		background: var(--bg-elevated);
		padding: 8px 16px;
		border: 1px solid var(--border-default);
		letter-spacing: 0.02em;
		width: 100%;
		text-align: center;
	}

	.rename-hint-sub {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--text-muted);
		text-align: center;
		margin: 0;
	}

	.rename-hint-close {
		font-family: var(--font-mono);
		font-size: 11px;
		font-weight: 600;
		color: var(--bg-base);
		background: var(--text-primary);
		border: 1px solid var(--text-primary);
		padding: 6px 24px;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		cursor: pointer;
		transition: all var(--transition-fast);
		margin-top: var(--space-xs);
	}

	.rename-hint-close:hover {
		background: var(--text-secondary);
		border-color: var(--text-secondary);
	}
</style>
