// Prueba minima de la ruta de entrega: ventana Win32 con una tabla y un boton.
#include <windows.h>
#include <commctrl.h>

static LRESULT CALLBACK Proc(HWND h, UINT m, WPARAM w, LPARAM l) {
    switch (m) {
    case WM_CREATE: {
        HFONT f = (HFONT)GetStockObject(DEFAULT_GUI_FONT);
        HWND lv = CreateWindowExW(WS_EX_CLIENTEDGE, WC_LISTVIEWW, L"",
            WS_CHILD | WS_VISIBLE | LVS_REPORT, 12, 12, 560, 150, h, (HMENU)1, nullptr, nullptr);
        SendMessageW(lv, WM_SETFONT, (WPARAM)f, TRUE);
        ListView_SetExtendedListViewStyle(lv, LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES);
        const wchar_t* cols[] = {L"Pieza", L"Perfil", L"Largo (mm)", L"Cantidad"};
        int anchos[] = {150, 190, 110, 90};
        for (int i = 0; i < 4; i++) {
            LVCOLUMNW c{}; c.mask = LVCF_TEXT | LVCF_WIDTH; c.cx = anchos[i];
            c.pszText = const_cast<wchar_t*>(cols[i]);
            ListView_InsertColumn(lv, i, &c);
        }
        const wchar_t* filas[2][4] = {{L"Larguero", L"Cuadrado 40x40x2", L"1250", L"8"},
                                      {L"Travesano", L"Rectangular 50x25x1.5", L"640", L"12"}};
        for (int r = 0; r < 2; r++) {
            LVITEMW it{}; it.mask = LVIF_TEXT; it.iItem = r; it.pszText = const_cast<wchar_t*>(filas[r][0]);
            ListView_InsertItem(lv, &it);
            for (int c = 1; c < 4; c++) ListView_SetItemText(lv, r, c, const_cast<wchar_t*>(filas[r][c]));
        }
        HWND b = CreateWindowExW(0, L"BUTTON", L"Calcular barras", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
            12, 176, 160, 32, h, (HMENU)2, nullptr, nullptr);
        SendMessageW(b, WM_SETFONT, (WPARAM)f, TRUE);
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
    wc.lpszClassName = L"PruebaEntrega";
    RegisterClassExW(&wc);
    HWND h = CreateWindowExW(0, L"PruebaEntrega", L"Prueba de entrega", WS_OVERLAPPEDWINDOW,
        60, 60, 610, 270, nullptr, nullptr, hi, nullptr);
    ShowWindow(h, show); UpdateWindow(h);
    MSG msg; while (GetMessageW(&msg, nullptr, 0, 0) > 0) { TranslateMessage(&msg); DispatchMessageW(&msg); }
    return 0;
}
