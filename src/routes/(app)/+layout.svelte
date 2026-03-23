<script lang="ts">
	import { onMount } from 'svelte';
	import { initializeSessionListeners, sessions } from '$lib/stores/sessions';
	import { getSessions } from '$lib/api';
	import { isTauri } from '$lib/ws';

	onMount(async () => {
		// Desktop (Tauri): initialize listeners and fetch sessions here
		// Browser/mobile: ConnectionScreen handles initialization after user connects
		if (isTauri()) {
			await initializeSessionListeners();

			const initialSessions = await getSessions();
			sessions.set(initialSessions);
		}
	});
</script>

<slot />
