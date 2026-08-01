<script lang="ts">
	import { page } from '$app/state';
	import '../app.css';
	import { auth, toasts } from '$lib/stores/app.svelte';
	import { onMount } from 'svelte';

	let { children } = $props();

	onMount(() => {
		if (!auth.token && page.url.pathname !== '/login') {
			window.location.href = '/login';
		}
	});

	// React to auth dropping (401 clears the token) while on a protected page.
	$effect(() => {
		if (!auth.token && page.url.pathname !== '/login') {
			window.location.href = '/login';
		}
	});
</script>

<div class="flex h-dvh overflow-hidden bg-zinc-950 text-zinc-100">
	{@render children()}

	<!-- Toasts -->
	<div class="pointer-events-none fixed bottom-4 left-1/2 z-50 flex w-full max-w-sm -translate-x-1/2 flex-col items-center gap-2 px-4">
		{#each toasts as t (t.id)}
			<div
				class="pointer-events-auto w-full rounded-xl border px-4 py-2.5 text-sm shadow-lg backdrop-blur {t.kind === 'success'
					? 'border-accent-500/40 bg-zinc-900/95 text-accent-300'
					: t.kind === 'error'
						? 'border-red-500/40 bg-zinc-900/95 text-red-300'
						: 'border-zinc-700 bg-zinc-900/95 text-zinc-200'}"
			>
				{t.message}
			</div>
		{/each}
	</div>
</div>
