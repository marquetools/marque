<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00018 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00411">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00411][Error] Vocabulary declarations must have a root from one of the built-in 
        types of 'datasphere', 'organization', 'individual', 'group', or 'role'. Declaration of custom
        root types are not permitted.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For all ntk:VocabularyType names, ensure that they are all linked to a built-in root vocabulary type.
    </sch:p>
    <sch:rule id="ISM-ID-00411-R1" context="ntk:VocabularyType[@ntk:name]">
        <sch:let name="root" value="substring-before(@ntk:name,':')"/>
        <sch:assert test="string-length($root)&gt;0" flag="error" role="error">
            [ISM-ID-00411][Error] Vocabulary declarations must have a root from one of the built-in 
            types of 'datasphere', 'organization', 'individual', 'group', or 'role'. Declaration of custom
            root types are not permitted.
        </sch:assert>
        <sch:assert test="some $value in ('datasphere', 'organization', 'individual', 'group', 'role') satisfies $root=$value" flag="error" role="error">
            [ISM-ID-00411][Error] Vocabulary declarations must have a root from one of the built-in 
            types of 'datasphere', 'organization', 'individual', 'group', or 'role'. The root vocabulary type 
            found [<sch:value-of select="$root"/>] is not valid.
        </sch:assert>
    </sch:rule>
</sch:pattern>
