export type DnsProviderId = 'alidns' | 'dnspod' | 'cloudflare' | 'google' | 'custom'

export const DNS_PROVIDERS: { value: DnsProviderId; label: string }[] = [
  { value: 'alidns', label: 'Ali DoH' },
  { value: 'dnspod', label: 'DNSPod' },
  { value: 'cloudflare', label: 'Cloudflare' },
  { value: 'google', label: 'Google' },
  { value: 'custom', label: 'Custom' },
]

export const dnsProviderLabel = (id: string | undefined): string => {
  const hit = DNS_PROVIDERS.find((p) => p.value === (id || 'alidns'))
  return hit ? hit.label : 'Custom'
}
