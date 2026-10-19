/* Wine 9 provides RtlGenRandom in advapi32 but has no cryptbase.dll.
 * Chromium loads cryptbase before resolving advapi32!SystemFunction036.
 * Keep randomness in Wine's implementation; this DLL only forwards exports.
 */
int __stdcall wintone_cryptbase_entry(void *module, unsigned long reason, void *reserved)
{
  (void)module;
  (void)reason;
  (void)reserved;
  return 1;
}
