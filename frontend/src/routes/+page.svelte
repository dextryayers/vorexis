<script lang="ts">
	import Sidebar from '$lib/components/Sidebar.svelte';
	import ScanComposer from '$lib/components/ScanComposer.svelte';
	import ScanLive from '$lib/components/ScanLive.svelte';
	import ScanResults from '$lib/components/ScanResults.svelte';
	import ChatMessages from '$lib/components/ChatMessages.svelte';
	import { api, streamSSE } from '$lib/api/client';
	import {
		activeScan,
		loadChats,
		loadScans,
		pushToast,
		setActiveScan
	} from '$lib/stores/app.svelte';
	import { onMount } from 'svelte';
	import type { Chat, ChatMessage, Scan, ScanEvent } from '$lib/types';

	let chatId: string | null = $state(null);
	let chat: Chat | null = $state(null);
	let messages: ChatMessage[] = $state([]);
	let scanning = $state(false);
	let scanEvents: ScanEvent[] = $state([]);
	let error = $state<string | null>(null);
	let streaming = $state(false);
	let viewScan: Scan | null = $state(null);
	let sidebarOpen = $state(false);
	let focusTick = $state(0);
	let stopStream: (() => void) | null = null;

	async function selectChat(id: string) {
		viewScan = null;
		scanning = false;
		chatId = id;
		chat = (await api<Chat[]>('/api/chat')).find((c) => c.id === id) ?? null;
		messages = await api<ChatMessage[]>(`/api/chat/${id}/messages`);
	}

	function newChat() {
		chatId = null;
		chat = null;
		messages = [];
		scanEvents = [];
		scanning = false;
		viewScan = null;
		setActiveScan(null);
	}

	async function deleteChat(id: string) {
		if (chatId === id) newChat();
	}

	function selectScan(scan: Scan) {
		setActiveScan(scan.id);
		viewScan = scan;
		scanning = scan.status === 'running';
		scanEvents = [];
	}

	async function streamAnswer(targetChatId: string, text: string) {
		streaming = true;
		messages.push({ role: 'assistant', content: '', created_at: new Date().toISOString() });
		try {
			const { stream, stop } = streamSSE('/api/chat/send/stream', {
				chat_id: targetChatId,
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
		if (streaming) return;
		error = null;
		let targetChatId = chatId;
		if (!targetChatId) {
			const created = await api<{ id: string }>('/api/chat', {
				method: 'POST',
				body: { title: text.slice(0, 48), scan_id: null }
			});
			targetChatId = created.id;
			chatId = targetChatId;
			loadChats();
		}
		messages.push({ role: 'user', content: text, created_at: new Date().toISOString() });
		await streamAnswer(targetChatId, text);
	}

	function handleStop() {
		stopStream?.();
	}

	async function handleScan(target: string, modules: string[], options: Record<string, string>) {
		if (streaming) return;
		error = null;
		scanning = true;
		scanEvents = [];
		viewScan = null;
		try {
			const scan = await api<Scan>('/api/scans', {
				method: 'POST',
				body: { target, modules, options }
			});
			setActiveScan(scan.id);
			if (!chatId) {
				const created = await api<{ id: string }>('/api/chat', {
					method: 'POST',
					body: { title: target.slice(0, 48), scan_id: scan.id }
				});
				chatId = created.id;
			}
			loadChats();
			loadScans();
			pushToast('Scan started', 'success');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			scanning = false;
		}
	}

	onMount(() => {
		loadScans();
		const onKey = (e: KeyboardEvent) => {
			if (e.key === '/' && !/^(input|textarea|select)$/i.test((e.target as HTMLElement)?.tagName ?? '')) {
				e.preventDefault();
				focusTick++;
			}
		};
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	});
</script>

<div class="flex h-dvh w-full overflow-hidden bg-zinc-950 text-zinc-100">
	<Sidebar
		{chatId}
		activeScanId={activeScan.id}
		onNewChat={newChat}
		onSelectChat={selectChat}
		onDeleteChat={deleteChat}
		onSelectScan={selectScan}
		mobileOpen={sidebarOpen}
		onCloseMobile={() => (sidebarOpen = false)}
	/>

	<main class="relative flex min-w-0 flex-1 flex-col">
		<!-- Mobile hamburger -->
		<button
			onclick={() => (sidebarOpen = true)}
			class="absolute left-3 top-3 z-20 flex h-9 w-9 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/80 text-zinc-300 backdrop-blur md:hidden"
			aria-label="Open sidebar"
		>
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M3 6h18M3 12h18M3 18h18" />
			</svg>
		</button>

		{#if viewScan}
			<ScanResults scan={viewScan} onBack={() => (viewScan = null)} />
		{:else if scanning}
			<ScanLive bind:events={scanEvents} bind:scanning />
		{:else}
			<ChatMessages {messages} {streaming} />
			<ScanComposer
				onSend={handleSend}
				onScan={handleScan}
				onStop={handleStop}
				{streaming}
				{error}
				{focusTick}
			/>
		{/if}
	</main>
</div>
