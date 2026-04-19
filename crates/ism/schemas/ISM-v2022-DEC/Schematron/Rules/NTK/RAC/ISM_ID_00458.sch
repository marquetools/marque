<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00052 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00458">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00458][Error] Restricted Authority Category NTK assertions must use the “datasphere” profile DES.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        ntk:AccessProfile elements that have an ntk:AccessPolicy child with the RAC value
        (urn:us:gov:ic:aces:ntk:rac) must have a ntk:ProfileDes with the datasphere value
        (urn:us:gov:ic:ntk:profile:datasphere).
    </sch:p>
    <sch:rule id="ISM-ID-00458-R1" context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:rac']/ntk:ProfileDes">
        <sch:assert test=". = 'urn:us:gov:ic:ntk:profile:datasphere'" flag="error" role="error">
            [ISM-ID-00458][Error] Restricted Authority Category NTK assertions must use the “datasphere” profile DES.
        </sch:assert>
    </sch:rule>
</sch:pattern>
