import("./styles/main.scss").then(()=> {
    console.debug("Loaded Styles...")
})
import("../pkg").then(client => {
    client.main()
    console.debug("Loaded Client...")
})

window.onload = ()=> {
    import('./engine').then(engine => {
        engine.load()
    })
}

if ('serviceWorker' in navigator) {
    window.addEventListener('load', () => {
        navigator.serviceWorker.register('/service-worker.js').then(registration => {
            console.log('SW registered: ', registration);
        }).catch(registrationError => {
            console.log('SW registration failed: ', registrationError);
        });
    });
}
