<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { api, streamSSE } from '$lib/api/client';
	import ChatMessages from '$lib/components/ChatMessages.svelte';
	import ScanComposer from '$lib/components/ScanComposer.svelte';
	import ScanLive from '$lib/components/ScanLive.svelte';
	import {
		loadChats,
		loadScans,
		pushToast,
		setActiveScan
	} from '$lib/stores/app.svelte';
	import { onDestroy } from 'svelte';
	import type { Chat, ChatMessage, Scan, ScanEvent } from '$lib/types';

	const chatId = $derived(page.params.id);

	let chat: Chat | null = $state(null);
	let messages: ChatMessage[] = $state([]);
	let error = $state<string | null>(null);
	let streaming = $state(false);
	let scanEvents: ScanEvent[] = $state([]);
	let scanning = $state(false);
	let scan: Scan | null = $state(null);
	let notFound = $state(false);
	let stopStream: (() => void) | null = null;

	function cleanDomain(u: string): string {
		return u.replace(/^https?:\/\//i, '').replace(/\/.*$/, '');
	}

	let domain: string | null = $state(null);

	$effect(() => {
		domain = scan ? cleanDomain(scan.target) : null;
	});

	async function loadChat() {
		try {
			chat = await api<Chat>(`/api/chat/${chatId}`);
			messages = await api<ChatMessage[]>(`/api/chat/${chatId}/messages`);
			if (chat.scan_id) {
				scan = await api<Scan>(`/api/scans/${chat.scan_id}`);
				setActiveScan(scan.id);
				scanning = scan.status === 'running';
				scanEvents = [];
				if (scanning) loadScans();
			} else {
				scan = null;
				setActiveScan(null);
			}
		} catch {
			notFound = true;
		}
	}

	$effect(() => {
		void chatId;
		notFound = false;
		chat = null;
		messages = [];
		scan = null;
		scanning = false;
		scanEvents = [];
		streaming = false;
		stopStream?.();
		stopStream = null;
		loadChat();
	});

	onDestroy(() => stopStream?.());

	async function streamAnswer(text: string) {
		streaming = true;
		messages.push({ role: 'assistant', content: '', created_at: new Date().toISOString() });
		try {
			const { stream, stop } = streamSSE('/api/chat/send/stream', {
				chat_id: chatId,
				message: text
			});
			stopStream = stop;
			const reader = stream.getReader();
			for (;;) {
				const { done, value } = await reader.read();
				if (done) break;
				if (value && typeof value === 'object') {
					const v = value as Record<string, unknown>;
					if (typeof v.delta === 'string') {
						const last = messages[messages.length - 1];
						messages[messages.length - 1] = { ...last, content: last.content + v.delta };
					}
					if (typeof v.error === 'string') {
						const last = messages[messages.length - 1];
						if (last && !last.content.trim()) messages.pop();
						pushToast(v.error, 'error');
						break;
					}
					if (v.done) break;
				}
			}
		} catch (e) {
			const aborted = e instanceof DOMException && e.name === 'AbortError';
			const last = messages[messages.length - 1];
			if (last && !last.content.trim()) messages.pop();
			if (!aborted) {
				pushToast(e instanceof Error ? e.message : String(e), 'error');
			}
		} finally {
			streaming = false;
			stopStream = null;
			loadChats();
		}
	}

	async function handleSend(text: string) {
		if (streaming || !chat) return;
		error = null;
		messages.push({ role: 'user', content: text, created_at: new Date().toISOString() });
		await streamAnswer(text);
	}

	function handleStop() {
		stopStream?.();
	}

	async function handleScan(target: string, modules: string[], options: Record<string, string>) {
		if (streaming || !chat) return;
		error = null;
		scanning = true;
		scanEvents = [];
		try {
			const s = await api<Scan>('/api/scans', {
				method: 'POST',
				body: { target, modules, options }
			});
			scan = s;
			setActiveScan(s.id);
			loadChats();
			loadScans();
			try {
				await api(`/api/chat/${chatId}/scan`, { method: 'POST', body: { scan_id: s.id } });
			} catch {
				pushToast('Scan started but could not be linked to this chat', 'error');
			}
			pushToast('Scan started', 'success');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			scanning = false;
		}
	}

	$effect(() => {
		if (scan && !scanning) {
			api<Scan>(`/api/scans/${scan.id}`).then((s) => {
				scan = s;
				loadScans();
			});
		}
	});
</script>

{#if notFound}
	<div class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 px-4 text-center">
		<div class="text-sm text-zinc-500">This chat session no longer exists.</div>
		<button
			onclick={() => goto('/')}
			class="rounded-xl border border-accent-500/40 bg-accent-500/10 px-4 py-2 text-sm font-medium text-accent-300 transition hover:bg-accent-500/20"
		>
			Start a new session
		</button>
	</div>
{:else if !chat}
	<div class="flex min-h-0 flex-1 items-center justify-center">
		<div class="h-8 w-8 animate-spin rounded-full border-2 border-zinc-700 border-t-accent-500"></div>
	</div>
{:else}
	<div class="flex min-h-0 flex-1 flex-col">
		<!-- Session header -->
		<div class="flex items-center gap-3 border-b border-zinc-800/80 px-4 py-2.5">
			<a
				href="/"
				title="New session"
				class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-zinc-800 text-zinc-400 transition hover:border-zinc-700 hover:text-zinc-200"
			>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
					<path d="M12 5v14M5 12h14" />
				</svg>
			</a>
			<div class="min-w-0 flex-1">
				<div class="flex items-center gap-2">
					<span class="truncate text-sm font-semibold text-zinc-100">{chat.title}</span>
					<span class="shrink-0 font-mono text-[10px] text-zinc-600">{chat.id.slice(0, 8)}</span>
				</div>
			</div>
			{#if domain}
				<a
					href={`/scan/${scan?.id}`}
					class="flex shrink-0 items-center gap-1.5 rounded-lg border border-accent-500/30 bg-accent-500/10 px-2.5 py-1 font-mono text-[11px] text-accent-300 transition hover:bg-accent-500/20"
				>
					{#if scanning}
						<span class="h-1.5 w-1.5 animate-pulse rounded-full bg-amber-400"></span>
					{/if}
					<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
						<path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
					</svg>
					{domain}
				</a>
			{/if}
		</div>

		{#if scanning && scan}
			<ScanLive bind:events={scanEvents} bind:scanning />
		{:else if scan && !scanning}
			<div class="border-b border-zinc-800/80 bg-zinc-900/30 px-4 py-2.5">
				<div class="mx-auto flex max-w-4xl flex-wrap items-center justify-between gap-2">
					<div class="flex flex-wrap items-center gap-2 text-[11px] text-zinc-400">
						<span
							class="rounded px-1.5 py-0.5 font-medium {scan.status === 'finished'
								? 'bg-accent-500/10 text-accent-300'
								: scan.status === 'stopped'
									? 'bg-zinc-800 text-zinc-400'
									: 'bg-red-500/10 text-red-300'}"
						>
							{scan.status}
						</span>
						<span class="font-mono">{scan.progress}%</span>
						<span>{scan.modules?.length ?? 0} modules</span>
						<span>{scan.summary?.total_events ?? 0} events</span>
					</div>
					<a
						href={`/scan/${scan.id}`}
						class="flex items-center gap-1.5 rounded-lg border border-accent-500/40 bg-accent-500/10 px-2.5 py-1.5 text-[11px] font-medium text-accent-300 transition hover:bg-accent-500/20"
					>
						View scan results
						<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
							<path d="M5 12h14M13 6l6 6-6 6" />
						</svg>
					</a>
				</div>
			</div>
		{/if}

		<ChatMessages {messages} {streaming} />
		<ScanComposer
			onSend={handleSend}
			onScan={handleScan}
			onStop={handleStop}
			{streaming}
			scanRunning={scanning}
			{error}
		/>
	</div>
{/if}
