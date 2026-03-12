<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="STRUCTURECHECK VALUECHECK"?>
<!-- Original rule id: NTK-ID-00013 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00406">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00406][Error] If Vocabulary Type is specified in an MN NTK assertion, it must specify 
        a version for either the issue (datasphere:mn:issue) or region (datasphere:mn:region) vocabularies.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If an ntk:VocabularyType element exists in an MN NTK assertion 
        (ntk:VocabularyType[../ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn']), then 
        (1) @ntk:name must be ‘datasphere:mn:issue’ or ‘datasphere:mn:region’ and 
        (2) the @ntk:sourceVersion attribute is required.
    </sch:p>
    <sch:rule id="ISM-ID-00406-R1" context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn']/ntk:VocabularyType">
        <sch:assert test="@ntk:sourceVersion" flag="error" role="error">
            [ISM-ID-00406][Error] The @ntk:sourceVersion attribute is required.
        </sch:assert>
        <sch:assert test="@ntk:name = 'datasphere:mn:issue' or @ntk:name = 'datasphere:mn:region'" flag="error" role="error">
            [ISM-ID-00406][Error] The name attribute must be ‘datasphere:mn:issue’ or ‘datasphere:mn:region’.
        </sch:assert>
    </sch:rule>
</sch:pattern>
