<script lang="ts">
	import DeviceList from '#lib/components/DeviceList.svelte';
</script>

<h1>ESPHome</h1>
<p class="note">
	Modules ESP32 (lecteur de badges, boîtier IR, NsPanel…). Ils sont encore connectés à Home
	Assistant : RustHome les lit via le miroir, et ne s’y connectera qu’au jalon M6, sur banc de test
	d’abord.
</p>
<DeviceList
	integrations={['esphome', 'ha:esphome']}
	columns={[{ label: 'Firmware', value: (d) => d.sw_version ?? undefined }]}
/>

<style>
	.note {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
</style>
