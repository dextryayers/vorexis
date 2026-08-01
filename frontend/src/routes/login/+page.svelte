<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$lib/api/client';
	import { login } from '$lib/stores/app.svelte';
	import type { AuthResponse } from '$lib/types';

	let mode = $state<'login' | 'register'>('login');
	let username = $state('');
	let email = $state('');
	let password = $state('');
	let loading = $state(false);
	let error = $state<string | null>(null);

	async function submit() {
		error = null;
		loading = true;
		try {
			if (mode === 'register') {
				await api('/api/auth/register', { method: 'POST', body: { username, email, password } });
			}
			const res = await api<AuthResponse>('/api/auth/login', {
				method: 'POST',
				body: { username, password }
			});
			login(res.access_token, res.username);
			goto('/');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}
</script>

<div class="flex min-h-dvh w-full items-center justify-center bg-zinc-950 px-4">
	<div class="w-full max-w-sm">
		<div class="mb-8 flex flex-col items-center">
			<div
				class="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-accent-500/10 text-accent-400 ring-1 ring-accent-500/30"
			>
				<svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
					<path d="M12 2l8 3.5v5.1c0 5-3.4 9.6-8 10.9-4.6-1.3-8-5.9-8-10.9V5.5L12 2z" />
					<path d="M9 12l2 2 4-4.5" />
				</svg>
			</div>
			<h1 class="text-xl font-semibold text-zinc-100">AIPentest</h1>
			<p class="mt-1 text-sm text-zinc-500">AI-powered web security scanning</p>
		</div>

		<div class="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-6">
			<div class="mb-5 grid grid-cols-2 gap-1 rounded-xl bg-zinc-800/70 p-1 text-sm">
				<button
					onclick={() => (mode = 'login')}
					class="rounded-lg py-1.5 font-medium transition {mode === 'login'
						? 'bg-zinc-700 text-zinc-100'
						: 'text-zinc-500 hover:text-zinc-300'}"
				>
					Login
				</button>
				<button
					onclick={() => (mode = 'register')}
					class="rounded-lg py-1.5 font-medium transition {mode === 'register'
						? 'bg-zinc-700 text-zinc-100'
						: 'text-zinc-500 hover:text-zinc-300'}"
				>
					Register
				</button>
			</div>

			<form onsubmit={(e) => { e.preventDefault(); submit(); }} class="space-y-3">
				{#if mode === 'register'}
					<input
						bind:value={email}
						type="email"
						required
						placeholder="Email"
						class="w-full rounded-xl border border-zinc-700 bg-zinc-800/60 px-3.5 py-2.5 text-sm text-zinc-100 placeholder-zinc-600 outline-none transition focus:border-accent-500/60"
					/>
				{/if}
				<input
					bind:value={username}
					required
					minlength="3"
					placeholder="Username"
					class="w-full rounded-xl border border-zinc-700 bg-zinc-800/60 px-3.5 py-2.5 text-sm text-zinc-100 placeholder-zinc-600 outline-none transition focus:border-accent-500/60"
				/>
				<input
					bind:value={password}
					type="password"
					required
					minlength="6"
					placeholder="Password"
					class="w-full rounded-xl border border-zinc-700 bg-zinc-800/60 px-3.5 py-2.5 text-sm text-zinc-100 placeholder-zinc-600 outline-none transition focus:border-accent-500/60"
				/>

				{#if error}
					<div class="rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-xs text-red-300">
						{error}
					</div>
				{/if}

				<button
					type="submit"
					disabled={loading}
					class="w-full rounded-xl bg-accent-600 py-2.5 text-sm font-semibold text-zinc-950 transition enabled:hover:bg-accent-500 disabled:opacity-40"
				>
					{loading ? 'Tunggu...' : mode === 'login' ? 'Masuk' : 'Buat akun'}
				</button>
			</form>
		</div>

		<p class="mt-6 text-center font-mono text-[11px] text-zinc-600">
			Rust engine &middot; FastAPI orkestrasi &middot; AI analysis
		</p>
	</div>
</div>
