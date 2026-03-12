<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00011 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00404">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00404][Error] The Access Profile Value for MN NTK assertions must use the appropriate
        subject or region vocabulary.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        Given an MN NTK assertion (ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn'), the
        ntk:AccessProfileValue elements @ntk:vocabulary attribute must be either
        'datasphere:mn:issue' or 'datasphere:mn:region'.
    </sch:p>
    <sch:rule id="ISM-ID-00404-R1" context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn']/ntk:AccessProfileValue">
        <sch:assert test="@ntk:vocabulary = 'datasphere:mn:issue' or @ntk:vocabulary = 'datasphere:mn:region'" flag="error" role="error">
            [ISM-ID-00404][Error] The Access Profile Value for MN NTK assertions must use the appropriate 
            subject or region vocabulary.</sch:assert>
    </sch:rule>
</sch:pattern>
