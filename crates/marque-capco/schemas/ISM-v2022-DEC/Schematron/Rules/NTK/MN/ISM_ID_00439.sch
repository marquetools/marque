<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00046 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00439">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00439][Error] When both issues (datasphere:mn:issue) and regions (datasphere:mn:region) are specified
        for in a Mission Need NTK instance, the version of the list specified for both must be the same.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For Mission Need profile NTK instances that have Vocabulary Types of both issue and region,
        verify that the @ntk:sourceVersion attribute values specified for both datasphere:mn:issue 
        and datasphere:mn:region are the same.
    </sch:p>
    <sch:rule id="ISM-ID-00439-R1" context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn'][ntk:VocabularyType[@ntk:name='datasphere:mn:issue'] and ntk:VocabularyType[@ntk:name='datasphere:mn:region']]">
        <sch:assert test="ntk:VocabularyType[@ntk:name='datasphere:mn:issue']/@ntk:sourceVersion = ntk:VocabularyType[@ntk:name='datasphere:mn:region']/@ntk:sourceVersion" flag="error" role="error">
            [ISM-ID-00439][Error] When both issues (datasphere:mn:issue) and regions (datasphere:mn:region) are specified
            for in a Mission Need NTK instance, the version of the list specified for both must be the same.
        </sch:assert>
    </sch:rule>
</sch:pattern>
