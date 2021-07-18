import("./client/src/styles/main.sass").then(()=> {
    console.debug("Loaded Styles...")
})
import("./client/pkg").then(client => {
    client.main()
    console.debug("Loaded Client...")
})
import("./engine/pkg").then(engine => {
    engine.main()
    console.debug("Loaded Engine...")
})


if ('serviceWorker' in navigator) {
    window.addEventListener('load', () => {
        navigator.serviceWorker.register('/service-worker.js').then(registration => {
            console.log('SW registered: ', registration);
        }).catch(registrationError => {

            console.log('SW registration failed: ', registrationError);
        });
    });
}
