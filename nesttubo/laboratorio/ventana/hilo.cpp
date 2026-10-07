// Prueba: hilo de trabajo + bandera de cancelacion, enlazado estatico.
#include <atomic>
#include <chrono>
#include <cstdio>
#include <mutex>
#include <thread>
int main() {
    std::atomic<bool> cancelar{false};
    std::atomic<long long> pasos{0};
    std::mutex m;
    int mejor = 99;
    std::thread t([&] {
        while (!cancelar.load()) {
            ++pasos;
            if (pasos % 1000000 == 0) { std::lock_guard<std::mutex> g(m); if (mejor > 1) --mejor; }
        }
    });
    std::this_thread::sleep_for(std::chrono::milliseconds(300));
    cancelar = true;
    t.join();
    std::printf("hilo ok: pasos=%lld mejor=%d\n", pasos.load(), mejor);
    return 0;
}
