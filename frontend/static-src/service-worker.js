self.addEventListener('notificationclick', function (event) {
    if (event.notification.data.action) {
        clients.openWindow(event.notification.data.action);
    }
});

self.addEventListener('push', function (event) {
    let data = event.data.json();

    let options = {
        body: data.message,
        icon: '/icon.png',
        //tag: data.channel,
        data: data.data,
        //renotify: true,
        vibrate: data.vibrate,
        silent: data.silent
    };

    console.log(JSON.stringify(event.data.json()))
    console.log(JSON.stringify(options))

    event.waitUntil(promiseChain);
});