<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Original rule id: NTK-ID-00012 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00405">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00405][Error] The Access Profile Value must not have an @ntk:qualifier attribute specified
        for MN NTK assertions.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        Given an MN NTK assertion (ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn'), the ntk:AccessProfileValue/@ntk:qualifier
        attribute is not allowed.
    </sch:p>
    <sch:rule id="ISM-ID-00405-R1" context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn']/ntk:AccessProfileValue">
        <sch:assert test="not(@ntk:qualifier)" flag="error" role="error">
            [ISM-ID-00405][Error] The Access Profile Value must not have an @ntk:qualifier attribute specified
            for MN NTK assertions.</sch:assert>
    </sch:rule>
</sch:pattern>
