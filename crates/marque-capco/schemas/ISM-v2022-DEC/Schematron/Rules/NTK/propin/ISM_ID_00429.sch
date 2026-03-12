<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00036 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00429">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00429][Error] PROPIN access policies must have characters after the predefined
      portion ‘urn:us:gov:ic:aces:ntk:propin:’.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Given an ntk:AccessPolicy that starts with ‘urn:us:gov:ic:aces:ntk:propin:’, the string
      length must be greater than 30 (that is, there must be characters after the predefined portion).
   </sch:p>
   <sch:rule id="ISM-ID-00429-R1" context="ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:propin:')]">
      <sch:assert test="string-length(.) &gt; 30" flag="error" role="error">
         [ISM-ID-00429][Error] PROPIN access policies must have characters after the predefined 
         portion ‘urn:us:gov:ic:aces:ntk:propin:’.</sch:assert>
   </sch:rule>
</sch:pattern>
