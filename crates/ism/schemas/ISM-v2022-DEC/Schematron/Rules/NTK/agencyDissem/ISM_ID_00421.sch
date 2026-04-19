<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK"?>
<!-- Original rule id: NTK-ID-00028 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00421">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00421][Error] An Agency Dissemination NTK must have one and only one entry
        qualified as the originator.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For every ntk:AccessProfile with an ntk:ProfileDes of [urn:us:gov:ic:ntk:profile:agencydissem], this rule ensures
        that it has one and only one ntk:AccessProfileValue element with an @ntk:qualifier of
        [originator].
    </sch:p>
    <sch:rule context="ntk:AccessProfile[ntk:ProfileDes='urn:us:gov:ic:ntk:profile:agencydissem']" id="ISM-ID-00421-R1">
        <sch:assert test="count(ntk:AccessProfileValue[@ntk:qualifier='originator']) = 1" flag="error" role="error">
            [ISM-ID-00421][Error] An Agency Dissemination NTK must have one and only one entry
            qualified as the originator.
        </sch:assert>
    </sch:rule>
</sch:pattern>
