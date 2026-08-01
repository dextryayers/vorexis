// ---------- Types shared with the backend API ----------

export type ModuleName =
	| 'port'
	| 'directory'
	| 'subdomain'
	| 'dns'
	| 'crawler'
	| 'parser'
	| 'http'
	| 'https'
	| 'tls'
	| 'ssl'
	| 'fuzzer'
	| 'waf'
	| 'fingerprint'
	| 'tech';

export interface ModuleMeta {
	label: string;
	description: string;
}

export interface AuthResponse {
	access_token: string;
	token_type: string;
	username: string;
}

export interface Scan {
	id: string;
	target: string;
	modules: string[];
	status: 'pending' | 'running' | 'finished' | 'stopped' | 'failed';
	progress: number;
	started_at: string | null;
	finished_at: string | null;
	summary: ScanSummary | null;
	error: string | null;
}

export interface ScanSummary {
	modules: { module: string; result_count: number; details: Record<string, unknown>[] }[];
	total_events: number;
}

export interface ScanEvent {
	id?: number;
	scan_id: string;
	module: string;
	event_type: 'event' | 'progress' | 'result' | 'complete' | 'done';
	data: unknown;
	duration_ms?: number;
	created_at?: string;
}

export interface Chat {
	id: string;
	title: string;
	scan_id: string | null;
	created_at: string;
	updated_at: string;
}

export interface ChatMessage {
	role: 'user' | 'assistant' | 'system';
	content: string;
	created_at?: string;
}

export interface Target {
	id: string;
	url: string;
	label: string | null;
	created_at: string;
}
