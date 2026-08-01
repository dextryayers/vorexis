<script lang="ts">
	// Renders engine result payloads per module into readable cards/tables.
	type Row = Record<string, unknown>;

	let { module, data, compact = false }: { module: string; data: unknown; compact?: boolean } = $props();

	const obj = $derived(asObj(data));

	function asObj(d: unknown): Row | null {
		return d && typeof d === 'object' && !Array.isArray(d) ? (d as Row) : null;
	}

	function list(d: unknown): Row[] {
		if (Array.isArray(d)) return d as Row[];
		if (d && typeof d === 'object') return [d as Row];
		return [];
	}
</script>

{#if obj}
	{#if module === 'port'}
		{#each list(obj['open_ports'] ?? obj['data'] ?? []) as p}
			<div class="flex flex-wrap items-center gap-2 py-0.5 font-mono text-xs">
				<span class="inline-flex items-center gap-1.5 rounded bg-accent-500/10 px-1.5 py-0.5 text-accent-300">
					<span class="h-1.5 w-1.5 rounded-full bg-accent-400"></span>
					{p['port']}/tcp
				</span>
				<span class="text-zinc-300">{String(p['service'] ?? '')}</span>
				{#if p['banner']}
					<span class="truncate text-zinc-500">{String(p['banner'])}</span>
				{/if}
				{#if p['latency_ms']}
					<span class="text-zinc-600">{p['latency_ms']}ms</span>
				{/if}
			</div>
		{/each}
	{:else if module === 'dns'}
		{#each list(obj['records']) as r}
			<div class="flex items-center gap-2 py-0.5 font-mono text-xs">
				<span class="w-12 shrink-0 rounded bg-zinc-800 px-1 py-0.5 text-center text-[10px] font-bold text-accent-400">{r['type']}</span>
				<span class="text-zinc-300">{String(r['value'] ?? '')}</span>
			</div>
		{/each}
	{:else if module === 'http' || module === 'https'}
		{#if !compact}
			<div class="grid grid-cols-2 gap-x-6 gap-y-1 text-xs">
				<div class="text-zinc-500">Status</div>
				<div class="font-mono text-zinc-200">{String(obj['status'] ?? '-')}</div>
				<div class="text-zinc-500">HTTP version</div>
				<div class="font-mono text-zinc-200">{String(obj['http_version'] ?? '-')}</div>
				<div class="text-zinc-500">Server</div>
				<div class="font-mono text-zinc-200">{String(obj['server'] ?? '-')}</div>
				<div class="text-zinc-500">X-Powered-By</div>
				<div class="font-mono text-zinc-200">{String(obj['x_powered_by'] ?? '-')}</div>
				<div class="text-zinc-500">Latency</div>
				<div class="font-mono text-zinc-200">{obj['latency_ms']}ms</div>
				<div class="text-zinc-500">Final URL</div>
				<div class="truncate font-mono text-zinc-200">{String(obj['final_url'] ?? '-')}</div>
			</div>
		{/if}
		{#if list(obj['methods']).length > 0}
			<div class="mt-2 flex flex-wrap items-center gap-1.5">
				<span class="text-[10px] font-semibold uppercase tracking-wider text-zinc-600">Methods</span>
				{#each list(obj['methods']) as mth}
					<span
						class="rounded border px-1.5 py-0.5 font-mono text-[10px] {Number(mth['status']) < 400
							? 'border-amber-500/40 bg-amber-500/10 text-amber-300'
							: 'border-zinc-700 bg-zinc-800/60 text-zinc-400'}"
						title={String(mth['note'] ?? '')}
					>
						{mth['method']} {mth['status']}
					</span>
				{/each}
			</div>
		{/if}
		{#if list(obj['security_headers']).length > 0}
			<div class="mt-2 space-y-1">
				{#each list(obj['security_headers']) as h}
					<div class="flex items-center gap-2 text-xs">
						<span class="font-mono text-zinc-400">{h['header']}</span>
						{#if h['present']}
							<span class="rounded bg-accent-500/10 px-1.5 py-0.5 text-[10px] text-accent-300">present</span>
						{:else}
							<span class="rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] text-amber-300">missing</span>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	{:else if module === 'tls' || module === 'ssl'}
		{#each list(obj['results']) as r}
			{#if r['error']}
				<div class="text-xs text-red-300">{String(r['error'])}</div>
			{:else}
				<div class="mb-1.5 flex flex-wrap items-center gap-2 text-xs">
					<span class="font-mono font-semibold text-accent-300">{String(r['host'] ?? '')}:{String(r['port'] ?? '')}</span>
					<span class="font-mono text-zinc-300">{String(r['tls_version'])}</span>
					<span class="font-mono text-zinc-400">{String(r['cipher_suite'])}</span>
					{#if r['supports_http2']}
						<span class="rounded bg-accent-500/10 px-1.5 py-0.5 text-[10px] text-accent-300">HTTP/2</span>
					{/if}
					{#if r['weak_cipher']}
						<span class="rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] text-amber-300">weak cipher</span>
					{/if}
				</div>
				{#each list(r['certificates']) as c}
					<div class="flex flex-wrap items-center gap-2 py-0.5 text-xs">
						<span class="font-mono text-zinc-300">{String(c['subject'] ?? '')}</span>
						{#if c['hostname_matches'] === false}
							<span class="rounded bg-red-500/10 px-1.5 py-0.5 text-[10px] text-red-300">hostname mismatch</span>
						{/if}
						{#if c['expired']}
							<span class="rounded bg-red-500/10 px-1.5 py-0.5 text-[10px] text-red-300">expired</span>
						{:else if c['expiring_soon']}
							<span class="rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] text-amber-300">{c['days_until_expiry']}d</span>
						{:else}
							<span class="text-zinc-600">{c['days_until_expiry']}d</span>
						{/if}
					</div>
				{/each}
			{/if}
		{/each}
	{:else if module === 'waf'}
		{#if list(obj['detected_wafs']).length > 0}
			{#each list(obj['detected_wafs']) as w}
				<div class="flex items-center gap-2 py-0.5 text-xs">
					<span class="rounded bg-red-500/10 px-1.5 py-0.5 font-mono font-semibold text-red-300">WAF</span>
					<span class="font-mono text-zinc-200">{String(w['name'])}</span>
					<span class="text-zinc-600">{String(w['confidence'])} confidence</span>
				</div>
			{/each}
		{:else if obj['probes_sent']}
			<div class="text-xs text-accent-300">
				No WAF detected — {obj['probes_sent']} probes, {obj['blocked_probes']} blocked
			</div>
		{/if}
	{:else if module === 'tech'}
		<div class="flex flex-wrap gap-1.5">
			{#each list(obj['technologies']) as t}
				<span
					class="rounded-md border border-zinc-700 bg-zinc-800/70 px-2 py-0.5 font-mono text-[11px] text-zinc-200"
				>
					{t['name']}
					<span class="text-zinc-600">· {t['type']}</span>
				</span>
			{/each}
		</div>
	{:else if module === 'fingerprint'}
		<div class="flex flex-wrap items-center gap-1.5">
			{#each list(obj['fingerprint']) as f}
				<span class="rounded-md bg-zinc-800 px-2 py-0.5 font-mono text-[11px] text-zinc-200">{String(f)}</span>
			{/each}
		</div>
	{:else if module === 'directory' || module === 'fuzzer'}
		{#if list(obj['found'] ?? obj['interesting']).length > 0}
			<div class="space-y-1">
				{#each list(obj['found'] ?? obj['interesting']).slice(0, compact ? 6 : 30) as f}
					<div class="flex flex-wrap items-center gap-2 font-mono text-xs">
						<span
							class="w-10 shrink-0 rounded px-1 py-0.5 text-center text-[10px] font-bold
							{Number(f['status']) < 400
								? 'bg-accent-500/10 text-accent-300'
								: 'bg-amber-500/10 text-amber-300'}"
						>{String(f['status'] ?? '-')}</span>
						<span class="truncate text-zinc-300">{String(f['url'] ?? '')}</span>
						{#if f['size']}
							<span class="shrink-0 text-zinc-600">{f['size']}B</span>
						{/if}
						{#if f['soft_404']}
							<span class="rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] text-amber-300">soft-404</span>
						{/if}
						{#if f['filtered']}
							<span class="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-500">filtered</span>
						{/if}
					</div>
				{/each}
			</div>
		{:else}
			<div class="text-xs text-zinc-600">No findings</div>
		{/if}
	{:else if module === 'crawler'}
		<div class="text-xs text-zinc-300">
			{obj['pages_crawled'] ?? 0} pages crawled from
			<span class="font-mono text-accent-400">{String(obj['target'] ?? '')}</span>
		</div>
		{#if !compact && list(obj['pages']).length > 0}
			<div class="mt-1 space-y-0.5">
				{#each list(obj['pages']).slice(0, 10) as p}
					<div class="flex items-center gap-2 font-mono text-[11px]">
						<span
							class="w-8 shrink-0 rounded px-1 text-center text-[10px] font-bold {Number(p['status']) < 400
								? 'bg-accent-500/10 text-accent-300'
								: 'bg-amber-500/10 text-amber-300'}"
						>{String(p['status'] ?? '-')}</span>
						<span class="truncate text-zinc-400">{String(p['url'] ?? '')}</span>
						{#if p['title']}
							<span class="truncate text-zinc-600">{String(p['title'])}</span>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	{:else if module === 'parser'}
		{#if obj['title']}
			<div class="mb-1 text-xs text-zinc-300">
				<span class="text-zinc-600">title:</span> {String(obj['title'])}
			</div>
		{/if}
		{#if list(obj['forms']).length > 0}
			<div class="space-y-1">
				{#each list(obj['forms']).slice(0, 5) as f}
					<div class="flex flex-wrap items-center gap-2 font-mono text-xs">
						<span class="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-amber-300">{String(f['method'])}</span>
						<span class="text-zinc-300">{String(f['action'] ?? '(self)')}</span>
						<span class="text-zinc-600">{list(f['inputs']).length} inputs</span>
						{#if f['insecure_action']}
							<span class="rounded bg-red-500/10 px-1.5 py-0.5 text-[10px] text-red-300">insecure action</span>
						{/if}
						{#if f['has_csrf_token'] === false}
							<span class="rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] text-amber-300">no CSRF</span>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
		{#if list(obj['comments']).length > 0}
			<div class="mt-1 space-y-0.5">
				{#each list(obj['comments']).slice(0, 3) as c}
					<div class="truncate font-mono text-[11px] text-zinc-500">&lt;!-- {String(c)} --&gt;</div>
				{/each}
			</div>
		{/if}
	{:else if module === 'subdomain'}
		{#if obj['wildcard_dns']}
			<div class="mb-1 rounded bg-amber-500/10 px-2 py-1 text-[10px] text-amber-300">
				Wildcard DNS detected — every subdomain resolves. Results may be unreliable.
			</div>
		{/if}
		{#if list(obj['resolved']).length > 0}
			<div class="space-y-0.5">
				{#each list(obj['resolved']) as s}
					<div class="flex items-center gap-2 font-mono text-xs">
						<span class="text-accent-300">{String(s['subdomain'])}</span>
						<span class="text-zinc-500">
							{Array.isArray(s['ips']) ? (s['ips'] as string[]).join(', ') : String(s['ips'] ?? '')}
						</span>
					</div>
				{/each}
			</div>
		{:else}
			<div class="text-xs text-zinc-600">No resolved subdomains</div>
		{/if}
	{:else}
		<pre class="overflow-x-auto font-mono text-[11px] text-zinc-400">{JSON.stringify(data, null, 2).slice(0, compact ? 500 : 2500)}</pre>
	{/if}
{:else}
	{#if data && typeof data === 'string'}
		<div class="text-xs text-zinc-400">{data}</div>
	{:else}
		<pre class="overflow-x-auto font-mono text-[11px] text-zinc-500">{JSON.stringify(data, null, 2).slice(0, 1200)}</pre>
	{/if}
{/if}
