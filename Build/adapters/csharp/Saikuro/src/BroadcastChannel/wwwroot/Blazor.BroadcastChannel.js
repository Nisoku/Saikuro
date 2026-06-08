const channels = {};

export function createOrJoin(connectionId, channelName) {
    if (!Object.hasOwn(channels, connectionId)) {
        channels[connectionId] = new BroadcastChannel(channelName);
    } else if (channels[connectionId].name !== channelName) {
        throw new Error('A connection with provided identifier has already been established to a different channel.');
    }

    return { Id: connectionId, Name: channels[connectionId].name };
}

export function close(connectionId) {
    if (Object.hasOwn(channels, connectionId)) {
        channels[connectionId].close();
        delete channels[connectionId];
    }
}

export function postMessage(connectionId, data) {
    if (Object.hasOwn(channels, connectionId)) {
        channels[connectionId].postMessage(data);
    }
}

export function addMessageEventListener(connectionId, dotNetBroadcastChannelReference) {
    if (Object.hasOwn(channels, connectionId)) {
        channels[connectionId].addEventListener('message', (event) => {
            dotNetBroadcastChannelReference.invokeMethodAsync('OnMessage', { Data: event.data, Origin: event.origin, LastEventId: event.lastEventId });
        });
    }
}