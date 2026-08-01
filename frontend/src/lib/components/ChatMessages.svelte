<script lang="ts">
	import Markdown from '$lib/components/Markdown.svelte';
	import { pushToast } from '$lib/stores/app.svelte';
	import type { ChatMessage } from '$lib/types';

	let { messages, streaming = false }: { messages: ChatMessage[]; streaming?: boolean } = $props();

	let container: HTMLDivElement;
	let stick = true;

	function onScroll() {
		const el = container;
		if (!el) return;
		stick = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
	}

	$effect(() => {
		void messages.length;
		void streaming;
		if (container && stick) container.scrollTop = container.scrollHeight;
	});

	function fmtTime(iso?: string): string {
		if (!iso) return '';
		const d = new Date(iso);
		if (Number.isNaN(d.getTime())) return '';
		return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
	}

	function copy(text: string) {
		navigator.clipboard?.writeText(text).then(
			() => pushToast('Copied to clipboard', 'success'),
			() => pushToast('Failed to copy', 'error')
		);
	}
</script>

<div bind:this={container} onscroll={onScroll} class="flex-1 overflow-y-auto">
	<div class="mx-auto flex w-full max-w-3xl flex-col px-4 py-6">
		{#if messages.length === 0}
			<div class="flex flex-1 flex-col items-center justify-center py-24 text-center">
				<div
					class="mb-5 flex h-14 w-14 items-center justify-center rounded-2xl bg-accent-500/10 ring-1 ring-accent-500/30"
				>
					<svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="text-accent-400">
						<path d="M12 2l8 3.5v5.1c0 5-3.4 9.6-8 10.9-4.6-1.3-8-5.9-8-10.9V5.5L12 2z" />
						<path d="M9 12l2 2 4-4.5" />
					</svg>
				</div>
				<h1 class="text-2xl font-semibold tracking-tight text-zinc-200">AIPentest</h1>
				<p class="mt-2 max-w-md text-sm leading-relaxed text-zinc-500">
					Scan web targets, review findings, and let AI analyze the results. Pick modules below,
					then send a target like
					<span class="font-mono text-accent-400">example.com</span>, or just ask a question.
				</p>
			</div>
		{:else}
			{#each messages as m, i}
				<div class="group mb-6 flex gap-3 {m.role === 'user' ? 'flex-row-reverse' : ''}">
					<div
						class="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg {m.role ===
						'user'
							? 'bg-zinc-800 text-zinc-300'
							: 'bg-accent-500/15 text-accent-400 ring-1 ring-accent-500/30'}"
					>
						{#if m.role === 'user'}
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
								<circle cx="12" cy="7" r="4" />
							</svg>
						{:else}
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
								<path d="M12 2l8 3.5v5.1c0 5-3.4 9.6-8 10.9-4.6-1.3-8-5.9-8-10.9V5.5L12 2z" />
							</svg>
						{/if}
					</div>
					<div
						class="max-w-[85%] rounded-2xl px-4 py-3 {m.role === 'user'
							? 'rounded-tr-sm bg-zinc-800 text-zinc-100'
							: 'rounded-tl-sm border border-zinc-800/80 bg-zinc-900/60'}"
					>
						<div class="mb-1 flex items-center gap-2">
							<span class="text-[10px] font-semibold uppercase tracking-widest {m.role === 'user' ? 'text-zinc-500' : 'text-accent-400'}">
								{m.role === 'user' ? 'You' : 'AIPentest'}
							</span>
							{#if m.created_at}
								<span class="text-[10px] text-zinc-600">{fmtTime(m.created_at)}</span>
							{/if}
						</div>
						{#if m.role === 'user'}
							<span class="whitespace-pre-wrap text-[15px] leading-relaxed">{m.content}</span>
						{:else}
							<div class="prose-ai text-[14.5px] text-zinc-300">
								<Markdown source={m.content} />
							</div>
						{/if}
						{#if m.role === 'assistant' && !streaming}
							<button
								onclick={() => copy(m.content)}
								title="Copy"
								class="mt-2 flex items-center gap-1 rounded-md border border-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-500 opacity-0 transition hover:border-zinc-700 hover:text-zinc-300 group-hover:opacity-100"
							>
								<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<rect x="9" y="9" width="13" height="13" rx="2" />
									<path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
								</svg>
								Copy
							</button>
						{/if}
						{#if streaming && i === messages.length - 1 && m.role === 'assistant'}
							<span class="ml-0.5 inline-block h-4 w-2 animate-pulse bg-accent-400 align-middle"></span>
						{/if}
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>
