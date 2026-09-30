# Vitul Examples

Start with [vitul-opentool](vitul-opentool/README.md). Its Window Lab shows an
overlay title bar, window operations through host IPC, English and Chinese
locales, several appearance themes, and business operations through OpenTool
JSON-RPC. The same example supports desktop WebViews and the AgentOS embedded
WebView.

[api](api/README.md) is the Vitul desktop API integration fixture used by CI. It
covers window, menu, tray, event, and other desktop API behavior; use
vitul-opentool as the starting point for new applications.

The remaining folders are focused runtime and API feature references. They are
useful when working on a specific compatible desktop capability; new
applications should follow the OpenTool and host API structure in
vitul-opentool.

See each directory's README for its build and run instructions.
