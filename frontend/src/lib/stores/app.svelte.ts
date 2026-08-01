import { getToken, getUsername, setToken, setUsername } from '$lib/api/client';
import type { Chat, Scan } from '$lib/types';

// ---------- Auth state ----------
export const auth = $state<{ token: string | null; username: string | null }>({
	token: typeof localStorage !== 'undefined' ? getToken() : null,
	username: typeof localStorage !== 'undefined' ? getUsername() : null
});

export function login(token: string, username: string) {
	setToken(token);
	setUsername(username);
	auth.token = token;
	auth.username = username;
}

export function logout() {
	setToken(null);
	setUsername(null);
	auth.token = null;
	auth.username = null;
}

// ---------- Chats ----------
export const chats = $state<Chat[]>([]);

export async function loadChats() {
	try {
		const { api } = await import('$lib/api/client');
		chats.length = 0;
		chats.push(...(await api<Chat[]>('/api/chat')));
	} catch {
		/* ignore */
	}
}

// ---------- Scans ----------
export const scans = $state<Scan[]>([]);
export const activeScan = $state<{ id: string | null }>({ id: null });

export function setActiveScan(id: string | null) {
	activeScan.id = id;
}

export async function loadScans() {
	try {
		const { api } = await import('$lib/api/client');
		scans.length = 0;
		scans.push(...(await api<Scan[]>('/api/scans')));
	} catch {
		/* ignore */
	}
}

export function upsertScan(scan: Scan) {
	const idx = scans.findIndex((s) => s.id === scan.id);
	if (idx >= 0) scans[idx] = scan;
	else scans.unshift(scan);
}

// ---------- Composer focus (global "/" shortcut) ----------
export const composerFocusTick = $state({ n: 0 });

export function bumpComposerFocus() {
	composerFocusTick.n++;
}

// ---------- Toasts ----------
export interface Toast {
	id: number;
	kind: 'success' | 'error' | 'info';
	message: string;
}

export const toasts = $state<Toast[]>([]);
let toastSeq = 0;

export function pushToast(message: string, kind: Toast['kind'] = 'info', ttl = 3500) {
	const id = ++toastSeq;
	toasts.push({ id, kind, message });
	setTimeout(() => {
		const idx = toasts.findIndex((t) => t.id === id);
		if (idx >= 0) toasts.splice(idx, 1);
	}, ttl);
}
