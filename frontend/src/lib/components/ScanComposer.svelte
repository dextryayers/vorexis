<script lang="ts">
	import { api } from '$lib/api/client';
	import { composerFocusTick } from '$lib/stores/app.svelte';
	import { onMount } from 'svelte';
	import type { ModuleMeta, ModuleName } from '$lib/types';

	let {
		onSend,
		onScan,
		onStop,
		streaming = false,
		scanRunning = false,
		error
	}: {
		onSend: (t: string) => void;
		onScan: (t: string, m: ModuleName[], o: Record<string, string>) => void;
		onStop: () => void;
		streaming?: boolean;
		scanRunning?: boolean;
		error: string | null;
	} = $props();

	let input = $state('');
	let modulesMeta = $state<Record<string, ModuleMeta>>({});
	let selected = $state<Set<ModuleName>>(new Set(['http', 'tls', 'tech', 'waf', 'fingerprint']));
	let sending = $state(false);
	let scanError = $state<string | null>(null);
	let textarea: HTMLTextAreaElement;

	const URL_RE = /^(https?):\/\//i;
	const HOST_RE = /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+(?::\d{1,5})?(?:\/.*)?$/i;
	const IP_RE = /^\d{1,3}(?:\.\d{1,3}){3}(?::\d{1,5})?(?:\/.*)?$/;
	const LOCAL_RE = /^localhost(?::\d{1,5})?(?:\/.*)?$/i;

	function looksLikeTarget(t: string): boolean {
		const s = t.trim();
		if (/\s/.test(s)) return false;
		return URL_RE.test(s) || IP_RE.test(s) || LOCAL_RE.test(s) || HOST_RE.test(s);
	}

	$effect(() => {
		scanError = error;
	});

	$effect(() => {
		if (composerFocusTick.n > 0) textarea?.focus();
	});

	const modules = $derived(Object.entries(modulesMeta));

	onMount(() => {
		textarea?.focus();
		api<Record<string, ModuleMeta>>('/api/chat/modules').then((m) => {
			modulesMeta = m;
		});
	});

	function toggle(m: ModuleName) {
		const next = new Set(selected);
		if (next.has(m)) next.delete(m);
		else next.add(m);
		selected = next;
	}

	async function submit() {
		const text = input.trim();
		if (!text || sending || streaming) return;
		if (looksLikeTarget(text)) {
			sending = true;
			try {
				await onScan(text, [...selected], {});
			} catch (e) {
				scanError = e instanceof Error ? e.message : String(e);
			} finally {
				sending = false;
			}
		} else {
			onSend(text);
		}
		input = '';
		queueMicrotask(resize);
	}

	function resize() {
		if (!textarea) return;
		textarea.style.height = 'auto';
		textarea.style.height = `${Math.min(textarea.scrollHeight, 160)}px`;
	}
</script>

<div class="border-t border-zinc-800/80 bg-zinc-950/80 px-4 pb-4 pt-3 backdrop-blur">
	{#if modules.length > 0}
		<div class="mx-auto mb-2.5 flex max-w-3xl flex-wrap gap-1.5">
			{#each modules as [name, meta]}
				<button
					onclick={() => toggle(name as ModuleName)}
					title={meta.description}
					class="rounded-full border px-2.5 py-1 text-[11px] font-medium transition {selected.has(name as ModuleName)
						? 'border-accent-500/50 bg-accent-500/15 text-accent-300'
						: 'border-zinc-800 bg-zinc-900/60 text-zinc-500 hover:border-zinc-700 hover:text-zinc-300'}"
				>
					{meta.label}
				</button>
			{/each}
		</div>
	{/if}

	{#if scanError}
		<div
			class="mx-auto mb-2 max-w-3xl rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-xs text-red-300"
		>
			{scanError}
		</div>
	{/if}

	<div
		class="mx-auto flex max-w-3xl items-end gap-2 rounded-2xl border border-zinc-700/80 bg-zinc-900/80 px-3 py-2.5 focus-within:border-accent-500/60 focus-within:ring-1 focus-within:ring-accent-500/30"
	>
	<textarea
		bind:this={textarea}
		rows="1"
		bind:value={input}
		oninput={resize}
		onkeydown={(e) => {
				if (e.key === 'Enter' && !e.shiftKey) {
					e.preventDefault();
					submit();
				}
			}}
			placeholder="Scan target (e.g. example.com) or ask the AI..."
			aria-label="Message input"
			class="max-h-40 flex-1 resize-none bg-transparent text-sm text-zinc-100 placeholder-zinc-600 outline-none"
		></textarea>
		{#if streaming}
			<button
				onclick={onStop}
				class="mb-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-red-500/90 text-zinc-950 transition hover:bg-red-400"
				title="Stop generating"
				aria-label="Stop generating"
			>
				<svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
					<rect x="6" y="6" width="12" height="12" rx="2" />
				</svg>
			</button>
		{:else}
			<button
				onclick={submit}
				disabled={!input.trim() || sending}
				class="mb-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-accent-600 text-zinc-950 transition enabled:hover:bg-accent-500 disabled:opacity-30"
				title="Scan / Send"
				aria-label="Send"
			>
				{#if sending}
					<span class="h-4 w-4 animate-spin rounded-full border-2 border-zinc-950/30 border-t-zinc-950"></span>
				{:else}
					<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4">
						<path d="M12 19V5M5 12l7-7 7 7" />
					</svg>
				{/if}
			</button>
		{/if}
	</div>
	{#if scanRunning}
		<p class="mx-auto mt-2 max-w-3xl text-center text-[10px] text-amber-400/80">
			Scan in progress — you can keep chatting, findings attach to this session automatically.
		</p>
	{:else}
		<p class="mx-auto mt-2 max-w-3xl text-center text-[10px] text-zinc-600">
			Scans run on the Rust engine &middot; results analyzed by AI (Ollama / OpenAI / Gemini / HuggingFace)
		</p>
	{/if}
</div>
