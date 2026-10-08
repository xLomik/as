// Prueba: literales anchos con tildes escritos tal cual en un fuente UTF-8.
// Si esto se ve bien bajo wine, la interfaz puede llevar tildes sin escapes.
#include <windows.h>
#include <commctrl.h>

static LRESULT CALLBACK Proc(HWND h, UINT m, WPARAM w, LPARAM l) {
    switch (m) {
    case WM_CREATE: {
        HFONT f = (HFONT)GetStockObject(DEFAULT_GUI_FONT);
        HWND lv = CreateWindowExW(WS_EX_CLIENTEDGE, WC_LISTVIEWW, L"",
            WS_CHILD | WS_VISIBLE | LVS_REPORT, 12, 12, 660, 110, h, (HMENU)1, nullptr, nullptr);
        SendMessageW(lv, WM_SETFONT, (WPARAM)f, TRUE);
        const wchar_t* cols[] = {L"Descripción", L"Ángulo izq. (°)", L"Separación (mm)", L"Tubería · ñ Ñ ü"};
        for (int i = 0; i < 4; i++) {
            LVCOLUMNW c{}; c.mask = LVCF_TEXT | LVCF_WIDTH; c.cx = 160;
            c.pszText = const_cast<wchar_t*>(cols[i]);
            ListView_InsertColumn(lv, i, &c);
        }
        LVITEMW it{}; it.mask = LVIF_TEXT; it.iItem = 0; it.pszText = const_cast<wchar_t*>(L"Travesaño");
        ListView_InsertItem(lv, &it);
        ListView_SetItemText(lv, 0, 1, const_cast<wchar_t*>(L"45"));
        HWND s = CreateWindowExW(0, L"STATIC", L"Mínimo demostrado: 12 barras · sobra menos de 1 barra",
            WS_CHILD | WS_VISIBLE, 12, 134, 500, 20, h, (HMENU)3, nullptr, nullptr);
        SendMessageW(s, WM_SETFONT, (WPARAM)f, TRUE);
        return 0; }
    case WM_DESTROY: PostQuitMessage(0); return 0;
    }
    return DefWindowProcW(h, m, w, l);
}

int WINAPI wWinMain(HINSTANCE hi, HINSTANCE, PWSTR, int show) {
    INITCOMMONCONTROLSEX ic{sizeof(ic), ICC_LISTVIEW_CLASSES | ICC_STANDARD_CLASSES};
    InitCommonControlsEx(&ic);
    WNDCLASSEXW wc{}; wc.cbSize = sizeof(wc); wc.lpfnWndProc = Proc; wc.hInstance = hi;
    wc.hCursor = LoadCursorW(nullptr, IDC_ARROW); wc.hbrBackground = (HBRUSH)(COLOR_BTNFACE + 1);
    wc.lpszClassName = L"PruebaAcentos";
    RegisterClassExW(&wc);
    HWND h = CreateWindowExW(0, L"PruebaAcentos", L"Cálculo de tubería", WS_OVERLAPPEDWINDOW,
        60, 60, 710, 210, nullptr, nullptr, hi, nullptr);
    ShowWindow(h, show); UpdateWindow(h);
    MSG msg; while (GetMessageW(&msg, nullptr, 0, 0) > 0) { TranslateMessage(&msg); DispatchMessageW(&msg); }
    return 0;
}
